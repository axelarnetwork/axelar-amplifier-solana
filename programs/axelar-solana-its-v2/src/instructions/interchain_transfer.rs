use crate::get_fee_and_decimals;
use crate::get_mint_decimals;
use crate::gmp::{GMPAccounts, ToGMPAccounts};
use crate::instructions::process_outbound;
use crate::program::AxelarSolanaItsV2;
use crate::state::{token_manager, FlowDirection};
use crate::{
    errors::ItsError,
    instructions::validate_token_manager_type,
    state::{current_flow_epoch, InterchainTokenService, TokenManager},
};
use anchor_lang::prelude::*;
use anchor_lang::system_program;
use anchor_spl::token_interface::TokenInterface;
use anchor_spl::token_interface::{Mint, TokenAccount};
use axelar_solana_gateway_v2::program::AxelarSolanaGatewayV2;
use interchain_token_transfer_gmp::GMPPayload;

#[derive(Accounts)]
#[event_cpi]
#[instruction(
	token_id: [u8; 32],
	destination_chain: String,
	destination_address: Vec<u8>,
	amount: u64,
	gas_value: u64,
	caller_program_id: Option<Pubkey>,
	caller_pda_seeds: Option<Vec<Vec<u8>>>,
	data: Option<Vec<u8>>,
)]
pub struct InterchainTransfer<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    //
    // Sender of the tokens
    //
    /// The account that wants to transfer - can be a direct signer or program
    pub authority: Signer<'info>,

    //
    // Gateway
    //
    /// CHECK: checked by the gateway program
    pub gateway_root_pda: UncheckedAccount<'info>,

    /// CHECK: signing PDA checked by gateway program
    pub gateway_event_authority: UncheckedAccount<'info>,

    /// Reference to the axelar gateway program
    pub gateway_program: Program<'info, AxelarSolanaGatewayV2>,

    /// CHECK: signing PDA checked by gateway program
    pub signing_pda: UncheckedAccount<'info>,

    //
    // Gas Service
    //

    // todo: replace with GasServiceAccounts
    /// CHECK: checked by the gas service program
    #[account(mut)]
    pub gas_treasury: UncheckedAccount<'info>,

    /// The GMP gas service program account
    pub gas_service:
        Program<'info, axelar_solana_gas_service_v2::program::AxelarSolanaGasServiceV2>,

    /// CHECK: checked by the gas service program
    pub gas_event_authority: UncheckedAccount<'info>,

    //
    // ITS
    //
    #[account(
        seeds = [InterchainTokenService::SEED_PREFIX],
        bump = its_root_pda.bump,
        constraint = !its_root_pda.paused @ ItsError::Paused,
    )]
    pub its_root_pda: Account<'info, InterchainTokenService>,

    #[account(
        mut,
        seeds = [
            TokenManager::SEED_PREFIX,
            its_root_pda.key().as_ref(),
            &token_id
        ],
        bump = token_manager_pda.bump,
        constraint = token_manager_pda.token_address == token_mint.key()  @ ItsError::InvalidTokenManagerPda
    )]
    pub token_manager_pda: Account<'info, TokenManager>,

    //
    // Token Info
    //
    pub token_program: Interface<'info, TokenInterface>,

    #[account(mint::token_program = token_program)]
    /// CHECK: We can't do futher checks here since it could be a canonical or a custom token
    pub token_mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        token::mint = token_mint,
        token::token_program = token_program,
        token::authority = authority,
    )]
    pub authority_token_account: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        associated_token::mint = token_mint,
        associated_token::token_program = token_program,
        associated_token::authority = token_manager_pda,
    )]
    pub token_manager_ata: InterfaceAccount<'info, TokenAccount>,

    //
    // Misc
    //
    pub system_program: Program<'info, System>,
}

impl<'info> ToGMPAccounts<'info> for InterchainTransfer<'info> {
    fn to_gmp_accounts(&self) -> GMPAccounts<'info> {
        GMPAccounts {
            payer: self.payer.to_account_info(),
            gateway_root_pda: self.gateway_root_pda.to_account_info(),
            gateway_program: self.gateway_program.to_account_info(),
            gas_treasury: self.gas_treasury.to_account_info(),
            gas_service: self.gas_service.to_account_info(),
            system_program: self.system_program.to_account_info(),
            its_root_pda: self.its_root_pda.clone(),
            call_contract_signing_pda: self.signing_pda.to_account_info(),
            its_program: self.program.to_account_info(),
            gateway_event_authority: self.gateway_event_authority.to_account_info(),
            gas_event_authority: self.gas_event_authority.to_account_info(),
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn interchain_transfer_handler(
    ctx: Context<InterchainTransfer>,
    token_id: [u8; 32],
    destination_chain: String,
    destination_address: Vec<u8>,
    amount: u64,
    gas_value: u64,
    caller_program_id: Option<Pubkey>,
    caller_pda_seeds: Option<Vec<Vec<u8>>>,
    data: Option<Vec<u8>>,
) -> Result<()> {
    let is_cpi = caller_program_id.is_some() && caller_pda_seeds.is_some();

    // TODO check security implications of the checks here
    if is_cpi {
        let caller_program_id = caller_program_id.unwrap();
        let caller_pda_seeds = caller_pda_seeds.unwrap();

        // The sender should be a PDA owned by the source program
        if ctx.accounts.authority.owner != &caller_program_id {
            msg!(
                "Sender account must be owned by the source program. Expected: {}, Got: {}",
                caller_program_id,
                ctx.accounts.authority.owner
            );
            return err!(ItsError::InvalidAccountData);
        }

        // Validate that the PDA can be derived using the provided seeds
        let seeds_refs: Vec<&[u8]> = caller_pda_seeds
            .iter()
            .map(std::vec::Vec::as_slice)
            .collect();
        let (expected_pda, _bump) =
            solana_program::pubkey::Pubkey::find_program_address(&seeds_refs, &caller_program_id);

        if expected_pda != *ctx.accounts.authority.key {
            msg!(
                "PDA derivation mismatch. Expected: {}, Got: {}",
                expected_pda,
                ctx.accounts.authority.key
            );
            return err!(ItsError::InvalidAccountData);
        }
    } else if ctx.accounts.authority.owner != &system_program::ID {
        return err!(ItsError::InvalidAccountOwner);
    }

    let source_address = *ctx.accounts.authority.key;
    process_outbound_transfer(
        ctx,
        token_id,
        destination_chain,
        destination_address,
        amount,
        gas_value,
        data,
        source_address,
    )
}

fn validate_cpi_authority(
    ctx: &Context<InterchainTransfer>,
    source_id: Pubkey,
    pda_seeds: &[Vec<u8>],
) -> Result<()> {
    // The sender should be a PDA owned by the source program
    if ctx.accounts.authority.owner != &source_id {
        msg!(
            "Sender account must be owned by the source program. Expected: {}, Got: {}",
            source_id,
            ctx.accounts.authority.owner
        );
        return err!(ItsError::InvalidAccountData);
    }

    // Validate that the PDA can be derived using the provided seeds
    let seeds_refs: Vec<&[u8]> = pda_seeds.iter().map(std::vec::Vec::as_slice).collect();
    let (expected_pda, _bump) =
        solana_program::pubkey::Pubkey::find_program_address(&seeds_refs, &source_id);

    if expected_pda != *ctx.accounts.authority.key {
        msg!(
            "PDA derivation mismatch. Expected: {}, Got: {}",
            expected_pda,
            ctx.accounts.authority.key
        );
        return err!(ItsError::InvalidAccountData);
    }

    Ok(())
}

fn validate_wallet_authority(ctx: &Context<InterchainTransfer>) -> Result<()> {
    if ctx.accounts.authority.owner != &system_program::ID {
        return err!(ItsError::InvalidAccountOwner);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn process_outbound_transfer(
    mut ctx: Context<InterchainTransfer>,
    token_id: [u8; 32],
    destination_chain: String,
    destination_address: Vec<u8>,
    mut amount: u64,
    gas_value: u64,
    data: Option<Vec<u8>>,
    source_address: Pubkey,
) -> Result<()> {
    let token_manager_account_info = ctx.accounts.token_manager_pda.clone();
    let amount_minus_fees = take_token(&mut ctx, &token_manager_account_info, amount)?;
    amount = amount_minus_fees;

    emit_cpi!(crate::events::InterchainTransfer {
        token_id,
        source_address,
        source_token_account: ctx.accounts.authority_token_account.key(),
        destination_chain: destination_chain.clone(),
        destination_address: destination_address.clone(),
        amount,
        data_hash: data
            .as_ref()
            .filter(|d| !d.is_empty())
            .map_or([0; 32], |d| solana_program::keccak::hash(d).0),
    });

    let inner_payload =
        GMPPayload::InterchainTransfer(interchain_token_transfer_gmp::InterchainTransfer {
            selector: interchain_token_transfer_gmp::InterchainTransfer::MESSAGE_TYPE_ID
                .try_into()
                .map_err(|_err| ProgramError::ArithmeticOverflow)?,
            token_id: token_id.into(),
            source_address: source_address.to_bytes().into(),
            destination_address: destination_address.into(),
            amount: alloy_primitives::U256::from(amount),
            data: data.unwrap_or_default().into(),
        });

    let gmp_accounts = ctx.accounts.to_gmp_accounts();
    process_outbound(gmp_accounts, destination_chain, gas_value, inner_payload)?;

    Ok(())
}

fn take_token(
    ctx: &mut Context<InterchainTransfer>,
    token_manager: &TokenManager,
    amount: u64,
) -> Result<u64> {
    use token_manager::Type::{
        LockUnlock, LockUnlockFee, MintBurn, MintBurnFrom, NativeInterchainToken,
    };

    validate_token_manager_type(
        ctx.accounts.token_manager_pda.ty,
        &ctx.accounts.token_mint.to_account_info(),
        &ctx.accounts.token_manager_pda.to_account_info(),
    )?;

    track_token_flow(ctx, amount, FlowDirection::Out)?;

    let transferred = match token_manager.ty {
        NativeInterchainToken | MintBurn | MintBurnFrom => {
            burn_from_source(ctx, amount)?;
            amount
        }
        LockUnlock => {
            let decimals = get_mint_decimals(&ctx.accounts.token_mint.to_account_info())?;
            transfer_to(ctx, amount, decimals)?;
            amount
        }
        LockUnlockFee => {
            let (fee, decimals) =
                get_fee_and_decimals(&ctx.accounts.token_mint.to_account_info(), amount)?;

            transfer_with_fee_to(ctx, amount, decimals, fee)?;

            amount
                .checked_sub(fee)
                .ok_or(ProgramError::ArithmeticOverflow)?
        }
    };

    Ok(transferred)
}

fn transfer_with_fee_to(
    ctx: &Context<InterchainTransfer>,
    amount: u64,
    decimals: u8,
    fee: u64,
) -> Result<()> {
    use anchor_spl::token_interface;

    let cpi_accounts = token_interface::TransferCheckedWithFee {
        token_program_id: ctx.accounts.token_program.to_account_info(),
        source: ctx.accounts.authority_token_account.to_account_info(),
        mint: ctx.accounts.token_mint.to_account_info(),
        destination: ctx.accounts.token_manager_ata.to_account_info(),
        authority: ctx.accounts.authority.to_account_info(),
    };

    let cpi_context = CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        cpi_accounts,
        &[],
    );

    token_interface::transfer_checked_with_fee(cpi_context, amount, decimals, fee)?;

    Ok(())
}

fn transfer_to(ctx: &Context<InterchainTransfer>, amount: u64, decimals: u8) -> Result<()> {
    use anchor_spl::token_interface;

    let cpi_accounts = token_interface::TransferChecked {
        from: ctx.accounts.authority_token_account.to_account_info(),
        mint: ctx.accounts.token_mint.to_account_info(),
        to: ctx.accounts.token_manager_ata.to_account_info(),
        authority: ctx.accounts.authority.to_account_info(),
    };

    let cpi_context = CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        cpi_accounts,
        &[],
    );

    token_interface::transfer_checked(cpi_context, amount, decimals)?;

    Ok(())
}

fn burn_from_source(ctx: &Context<InterchainTransfer>, amount: u64) -> Result<()> {
    use anchor_spl::token_interface;

    let cpi_accounts = token_interface::Burn {
        mint: ctx.accounts.token_mint.to_account_info(),
        from: ctx.accounts.authority_token_account.to_account_info(),
        authority: ctx.accounts.authority.to_account_info(),
    };

    let cpi_context = CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        cpi_accounts,
        &[],
    );

    token_interface::burn(cpi_context, amount)?;

    Ok(())
}

fn track_token_flow(
    ctx: &mut Context<InterchainTransfer>,
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
        msg!("Flow slot reset");
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
