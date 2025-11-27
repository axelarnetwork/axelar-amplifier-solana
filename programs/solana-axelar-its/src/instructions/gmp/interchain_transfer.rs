use crate::{
    errors::ItsError,
    events::InterchainTransferReceived,
    executable::{
        builder::AxelarExecuteWithInterchainToken, AxelarExecuteWithInterchainTokenInstruction,
        AxelarExecuteWithInterchainTokenPayload,
    },
    state::{
        current_flow_epoch, token_manager, FlowDirection, InterchainTokenService,
        InterchainTransferExecute, TokenManager,
    },
};
use anchor_lang::solana_program;
use anchor_lang::{prelude::*, InstructionData};
use anchor_spl::{
    associated_token::AssociatedToken,
    token_2022::spl_token_2022::{
        extension::{
            transfer_fee::TransferFeeConfig, BaseStateWithExtensions, StateWithExtensions,
        },
        state::Mint as SplMint,
    },
    token_interface::{Mint, TokenAccount, TokenInterface},
};
use solana_axelar_gateway::{payload::AxelarMessagePayload, Message};
use solana_program::{program_option::COption, program_pack::Pack};

#[derive(Accounts)]
#[event_cpi]
#[instruction(message: Message, source_chain: String, source_address: Vec<u8>, destination_address: Pubkey, token_id: [u8; 32], amount: u64, data: Vec<u8>)]
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
        constraint = destination.key() == destination_address
            @ ItsError::InvalidDestinationAddressAccount,
    )]
    pub destination: UncheckedAccount<'info>,

    #[account(
        init_if_needed,
        payer = payer,
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
            @ ItsError::TokenMintTokenManagerMissmatch
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

    pub associated_token_program: Program<'info, AssociatedToken>,

    pub system_program: Program<'info, System>,

    #[account(
        seeds = [
            InterchainTransferExecute::SEED_PREFIX,
            destination.key().as_ref(),
        ],
        bump,
    )]
    pub interchain_transfer_execute: Option<UncheckedAccount<'info>>,
}

#[allow(clippy::too_many_arguments)]
#[allow(clippy::unimplemented)]
pub fn execute_interchain_transfer_handler<'info>(
    mut ctx: Context<'_, '_, '_, 'info, ExecuteInterchainTransfer<'info>>,
    message: Message,
    source_chain: String,
    source_address: Vec<u8>,
    destination_address: Pubkey,
    token_id: [u8; 32],
    amount: u64,
    data: Vec<u8>,
) -> Result<()> {
    msg!("ExecuteInterchainTransfer handler");

    if amount == 0 {
        return err!(ItsError::InvalidAmount);
    }

    validate_token_manager_type(
        ctx.accounts.token_manager_pda.ty,
        &ctx.accounts.token_mint.to_account_info(),
        &ctx.accounts.token_manager_pda.to_account_info(),
    )?;

    let destination_token_account = ctx.accounts.destination_ata.key();
    let transferred_amount = handle_give_token_transfer(&mut ctx, amount)?;

    let data_hash = if data.is_empty() {
        [0; 32]
    } else {
        solana_program::keccak::hash(data.as_ref()).0
    };

    emit_cpi!(InterchainTransferReceived {
        command_id: message.command_id(),
        token_id,
        source_chain: source_chain.clone(),
        source_address: source_address.clone(),
        destination_address,
        destination_token_account,
        amount: transferred_amount,
        data_hash,
    });

    if !data.is_empty() {
        // Validate accounts

        let Some(interchain_transfer_execute) = ctx.accounts.interchain_transfer_execute.as_ref()
        else {
            return err!(ItsError::InterchainTransferExecutePdaMissing);
        };

        // Validate and decode payload data value

        msg!("Got interchain transfer data, length: {}", data.len());

        let destination_payload = AxelarMessagePayload::decode(&data)?;
        let destination_accounts = destination_payload.account_meta();

        if destination_accounts.len() != ctx.remaining_accounts.len() {
            return Err(ProgramError::NotEnoughAccountKeys.into());
        }

        let remaining_metas = ctx
            .remaining_accounts
            .iter()
            .map(|ai| AccountMeta {
                pubkey: ai.key(),
                is_signer: ai.is_signer,
                is_writable: ai.is_writable,
            })
            .collect::<Vec<_>>();

        if !destination_accounts.eq(&remaining_metas) {
            msg!("Provided executable accounts do not match the payload specified accounts");
            return err!(ItsError::InvalidAccountData);
        }

        // Remove accounts from the final data sent to the destination program
        let destination_data = destination_payload.payload_without_accounts().to_vec();

        // Prepare instruction to invoke

        let accounts = AxelarExecuteWithInterchainToken {
            token_program: ctx.accounts.token_program.to_account_info(),
            token_mint: ctx.accounts.token_mint.to_account_info(),
            destination_program_ata: ctx.accounts.destination_ata.to_account_info(),
            interchain_transfer_execute: interchain_transfer_execute.to_account_info(),
        };

        let mut ix_accounts = accounts.to_account_metas(Some(true));
        ix_accounts.extend(destination_accounts);

        let ix_data = AxelarExecuteWithInterchainTokenInstruction {
            execute_payload: AxelarExecuteWithInterchainTokenPayload {
                command_id: message.command_id(),
                source_chain,
                source_address,
                token_id,
                token_mint: ctx.accounts.token_mint.key(),
                amount: transferred_amount,
                data: destination_data,
            },
        };

        let ix = solana_program::instruction::Instruction {
            program_id: ctx.accounts.destination.key(),
            accounts: ix_accounts,
            data: ix_data.data(),
        };

        let mut account_infos = accounts.to_account_infos();
        account_infos.extend(ctx.remaining_accounts.iter().cloned());

        let (_, axelar_transfer_execute_bump) =
            InterchainTransferExecute::find_pda(ctx.accounts.destination.key);

        // Invoke the destination program

        solana_program::program::invoke_signed(
            &ix,
            &account_infos,
            // Sign with the interchain transfer execute PDA
            &[&[
                InterchainTransferExecute::SEED_PREFIX,
                ctx.accounts.destination.key().as_ref(),
                &[axelar_transfer_execute_bump],
            ]],
        )?;
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
                .ok_or(ItsError::ArithmeticOverflow)?
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

pub fn get_fee_and_decimals(token_mint: &AccountInfo, amount: u64) -> Result<(u64, u8)> {
    let mint_data = token_mint.try_borrow_data()?;
    let mint_state = StateWithExtensions::<SplMint>::unpack(&mint_data)?;
    let fee_config = mint_state.get_extension::<TransferFeeConfig>()?;
    let epoch = Clock::get()?.epoch;

    let fee = fee_config
        .calculate_epoch_fee(epoch, amount)
        .ok_or(ItsError::ArithmeticOverflow)?;
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
) -> Result<()> {
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
            err!(ItsError::InvalidInstructionData)
        }
        (
            COption::Some(key),
            token_manager::Type::NativeInterchainToken
            | token_manager::Type::MintBurn
            | token_manager::Type::MintBurnFrom,
        ) if &key != token_manager_pda.key => {
            msg!("TokenManager is not the mint authority, which is required for this token manager type");
            err!(ItsError::InvalidInstructionData)
        }
        _ => Ok(()),
    }
}
