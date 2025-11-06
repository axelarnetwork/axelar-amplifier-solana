use crate::{
    errors::ItsError,
    events::InterchainTransferReceived,
    state::{
        current_flow_epoch, token_manager, FlowDirection, InterchainTokenService, TokenManager,
    },
};
use anchor_lang::prelude::*;
use anchor_lang::solana_program;
use anchor_spl::{
    token_2022::spl_token_2022::{
        extension::{
            transfer_fee::TransferFeeConfig, BaseStateWithExtensions, StateWithExtensions,
        },
        state::Mint as SplMint,
    },
    token_interface::{Mint, TokenAccount, TokenInterface},
};
use solana_axelar_gateway::Message;
use solana_program::{entrypoint::ProgramResult, program_option::COption, program_pack::Pack};

#[derive(Accounts)]
#[event_cpi]
#[instruction(token_id: [u8; 32], source_address: String, destination_address: Pubkey, amount: u64, data: Vec<u8>, message: Message, source_chain: String)]
pub struct ExecuteInterchainTransfer<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        seeds = [InterchainTokenService::SEED_PREFIX],
        bump = its_root_pda.bump,
        constraint = !its_root_pda.paused @ ItsError::Paused,
        signer, // important: only ITS can call this
    )]
    pub its_root_pda: Account<'info, InterchainTokenService>,

    /// CHECK: we check this matches the destination address
    #[account(
        mut,
        constraint = destination.key() == destination_address
            @ ItsError::InvalidDestinationAddressAccount)]
    pub destination: UncheckedAccount<'info>,

    #[account(
        mut,
        associated_token::mint = token_mint,
        associated_token::authority = destination,
        associated_token::token_program = token_program
    )]
    pub destination_ata: InterfaceAccount<'info, TokenAccount>,

    #[account(mut, mint::token_program = token_program)]
    /// CHECK: We can't do further checks here since it could be a canonical or a custom token
    pub token_mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        seeds = [
            TokenManager::SEED_PREFIX,
            its_root_pda.key().as_ref(),
            &token_id,
        ],
        bump = token_manager_pda.bump,
        constraint = token_manager_pda.token_address == token_mint.key()
            @ ItsError::InvalidTokenManagerPda
    )]
    pub token_manager_pda: Account<'info, TokenManager>,

    #[account(
        mut,
        associated_token::mint = token_mint,
        associated_token::authority = token_manager_pda,
        associated_token::token_program = token_program
    )]
    pub token_manager_ata: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
}

#[allow(clippy::too_many_arguments)]
pub fn execute_interchain_transfer_handler(
    mut ctx: Context<ExecuteInterchainTransfer>,
    token_id: [u8; 32],
    source_address: String,
    destination_address: Pubkey,
    amount: u64,
    data: Vec<u8>,
    message: Message,
    source_chain: String,
) -> Result<()> {
    validate_token_manager_type(
        ctx.accounts.token_manager_pda.ty,
        &ctx.accounts.token_mint.to_account_info(),
        &ctx.accounts.token_manager_pda.to_account_info(),
    )?;

    let destination_token_account = ctx.accounts.destination.key();
    let transferred_amount = handle_give_token_transfer(&mut ctx, amount)?;

    let data_hash = if data.is_empty() {
        [0; 32]
    } else {
        solana_program::keccak::hash(data.as_ref()).0
    };

    emit_cpi!(InterchainTransferReceived {
        command_id: message.command_id(),
        token_id,
        source_chain,
        source_address: source_address.as_bytes().to_vec(),
        destination_address,
        destination_token_account,
        amount: transferred_amount,
        data_hash,
    });

    if !data.is_empty() {
        // TODO invoke signed for execute with token
        msg!("ExecuteWithInterchainToken is not yet implemented");
    }

    Ok(())
}

fn handle_give_token_transfer(
    ctx: &mut Context<ExecuteInterchainTransfer>,
    amount: u64,
) -> Result<u64> {
    use token_manager::Type::{
        LockUnlock, LockUnlockFee, MintBurn, MintBurnFrom, NativeInterchainToken,
    };

    track_token_flow(ctx, amount, FlowDirection::In)?;
    let token_manager = &ctx.accounts.token_manager_pda;
    let token_id = token_manager.token_id;
    let token_manager_pda_bump = token_manager.bump;

    let its_root_key = ctx.accounts.its_root_pda.key();
    let bump_seed = [token_manager_pda_bump];
    let signer_seeds: &[&[&[u8]]] = &[&[
        TokenManager::SEED_PREFIX,
        its_root_key.as_ref(),
        token_id.as_ref(),
        &bump_seed,
    ]];

    let transferred = match token_manager.ty {
        NativeInterchainToken | MintBurn | MintBurnFrom => {
            mint_to_receiver(ctx, token_id, amount, token_manager_pda_bump)?;
            amount
        }
        LockUnlock => {
            let decimals = get_mint_decimals(&ctx.accounts.token_mint.to_account_info())?;
            transfer_to(ctx, amount, decimals, signer_seeds)?;
            amount
        }
        LockUnlockFee => {
            let (fee, decimals) =
                get_fee_and_decimals(&ctx.accounts.token_mint.to_account_info(), amount)?;

            transfer_with_fee_to(ctx, amount, decimals, fee, signer_seeds)?;

            amount
                .checked_sub(fee)
                .ok_or(ProgramError::ArithmeticOverflow)?
        }
    };

    Ok(transferred)
}

fn track_token_flow(
    ctx: &mut Context<ExecuteInterchainTransfer>,
    amount: u64,
    direction: FlowDirection,
) -> Result<()> {
    if ctx
        .accounts
        .token_manager_pda
        .flow_slot
        .flow_limit
        .is_none()
    {
        return Ok(());
    }

    // Reset the flow slot upon epoch change.
    let current_epoch = current_flow_epoch()?;
    if ctx.accounts.token_manager_pda.flow_slot.epoch != current_epoch {
        ctx.accounts.token_manager_pda.flow_slot.flow_in = 0;
        ctx.accounts.token_manager_pda.flow_slot.flow_out = 0;
        ctx.accounts.token_manager_pda.flow_slot.epoch = current_epoch;
    }

    ctx.accounts
        .token_manager_pda
        .flow_slot
        .add_flow(amount, direction)?;

    Ok(())
}

pub fn get_fee_and_decimals(
    token_mint: &AccountInfo,
    amount: u64,
) -> std::result::Result<(u64, u8), ProgramError> {
    let mint_data = token_mint.try_borrow_data()?;
    let mint_state = StateWithExtensions::<SplMint>::unpack(&mint_data)?;
    let fee_config = mint_state.get_extension::<TransferFeeConfig>()?;
    let epoch = Clock::get()?.epoch;

    let fee = fee_config
        .calculate_epoch_fee(epoch, amount)
        .ok_or(ProgramError::ArithmeticOverflow)?;
    Ok((fee, mint_state.base.decimals))
}

fn transfer_to(
    ctx: &Context<ExecuteInterchainTransfer>,
    amount: u64,
    decimals: u8,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    use anchor_spl::token_interface;

    let cpi_accounts = token_interface::TransferChecked {
        from: ctx.accounts.token_manager_ata.to_account_info(),
        mint: ctx.accounts.token_mint.to_account_info(),
        to: ctx.accounts.destination_ata.to_account_info(),
        authority: ctx.accounts.token_manager_pda.to_account_info(),
    };

    let cpi_context = CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        cpi_accounts,
        signer_seeds,
    );

    token_interface::transfer_checked(cpi_context, amount, decimals)?;

    Ok(())
}

fn transfer_with_fee_to(
    ctx: &Context<ExecuteInterchainTransfer>,
    amount: u64,
    decimals: u8,
    fee: u64,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    use anchor_spl::token_interface;

    let cpi_accounts = token_interface::TransferCheckedWithFee {
        token_program_id: ctx.accounts.token_program.to_account_info(),
        source: ctx.accounts.token_manager_ata.to_account_info(),
        mint: ctx.accounts.token_mint.to_account_info(),
        destination: ctx.accounts.destination_ata.to_account_info(),
        authority: ctx.accounts.token_manager_pda.to_account_info(),
    };

    let cpi_context = CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        cpi_accounts,
        signer_seeds,
    );

    token_interface::transfer_checked_with_fee(cpi_context, amount, decimals, fee)?;

    Ok(())
}

pub fn get_mint_decimals(token_mint: &AccountInfo) -> std::result::Result<u8, ProgramError> {
    let mint_data = token_mint.try_borrow_data()?;
    let mint_state = StateWithExtensions::<SplMint>::unpack(&mint_data)?;
    Ok(mint_state.base.decimals)
}

fn mint_to_receiver(
    ctx: &Context<ExecuteInterchainTransfer>,
    token_id: [u8; 32],
    initial_supply: u64,
    token_manager_bump: u8,
) -> Result<()> {
    use anchor_spl::token_interface;

    let cpi_accounts = token_interface::MintTo {
        mint: ctx.accounts.token_mint.to_account_info(),
        to: ctx.accounts.destination_ata.to_account_info(),
        authority: ctx.accounts.token_manager_pda.to_account_info(),
    };

    // Create signer seeds with proper lifetimes
    let its_root_key = ctx.accounts.its_root_pda.key();
    let bump_seed = [token_manager_bump];
    let signer_seeds: &[&[&[u8]]] = &[&[
        TokenManager::SEED_PREFIX,
        its_root_key.as_ref(),
        token_id.as_ref(),
        &bump_seed,
    ]];

    let cpi_context = CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        cpi_accounts,
        signer_seeds,
    );

    token_interface::mint_to(cpi_context, initial_supply)?;

    Ok(())
}

pub fn validate_token_manager_type(
    ty: token_manager::Type,
    token_mint: &AccountInfo,
    token_manager_pda: &AccountInfo,
) -> ProgramResult {
    let mint_data = token_mint.try_borrow_data()?;
    let mint = SplMint::unpack_from_slice(&mint_data)?;

    match (mint.mint_authority, ty) {
        (
            COption::None,
            token_manager::Type::NativeInterchainToken
            | token_manager::Type::MintBurn
            | token_manager::Type::MintBurnFrom,
        ) => {
            msg!("Mint authority is required for the given token manager type");
            Err(ProgramError::InvalidInstructionData)
        }
        (
            COption::Some(key),
            token_manager::Type::NativeInterchainToken
            | token_manager::Type::MintBurn
            | token_manager::Type::MintBurnFrom,
        ) if &key != token_manager_pda.key => {
            msg!("TokenManager is not the mint authority, which is required for this token manager type");
            Err(ProgramError::InvalidInstructionData)
        }
        _ => Ok(()),
    }
}
