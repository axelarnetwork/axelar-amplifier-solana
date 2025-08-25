use crate::state::Config;
use anchor_lang::prelude::*;
use anchor_lang::solana_program::log::sol_log_data;
use anchor_spl::token_interface::{self, Mint, TokenAccount, TokenInterface, TransferChecked};
use axelar_solana_gas_service_events::event_prefixes;

/// Pay gas fees for a contract call using SPL tokens.
///
/// Accounts expected:
/// 0. `[signer, writable]` The account (`sender`) paying the gas fee in SPL tokens.
/// 1. `[writable]` The sender's associated token account for the mint.
/// 2. `[writable]` The `config_pda` account.
/// 3. `[writable]` The config PDA's associated token account for the mint.
/// 4. `[]` The mint account for the SPL token.
/// 5. `[]` The SPL token program.
/// 6+. `[signer, writable]` Optional additional accounts required by the SPL token program for the transfer.
#[derive(Accounts)]
pub struct PaySplForContractCall<'info> {
    #[account(mut)]
    pub sender: Signer<'info>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = sender,
    )]
    pub sender_ata: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [
            Config::SEED_PREFIX,
        ],
        bump = config_pda.load()?.bump,
    )]
    pub config_pda: AccountLoader<'info, Config>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = config_pda,
    )]
    pub config_pda_ata: InterfaceAccount<'info, TokenAccount>,

    pub mint: InterfaceAccount<'info, Mint>,

    pub token_program: Interface<'info, TokenInterface>,
}

pub fn pay_spl_for_contract_call<'info>(
    // explicitly specify all lifetimes to fix the `remaining_accounts` issue
    // see more: https://solana.stackexchange.com/questions/20176/anchor-lifetime-may-not-live-long-enough-in-loop-of-remaining-accounts
    ctx: Context<'_, '_, '_, 'info, PaySplForContractCall<'info>>,
    destination_chain: String,
    destination_address: String,
    payload_hash: [u8; 32],
    params: &[u8],
    gas_fee_amount: u64,
    decimals: u8,
    refund_address: Pubkey,
) -> Result<()> {
    if gas_fee_amount == 0 {
        msg!("Gas fee amount cannot be zero");
        return Err(ProgramError::InvalidInstructionData.into());
    }

    // TODO(v2) check if this check is necessary
    // or move to account constraint
    if decimals != ctx.accounts.mint.decimals {
        msg!("Decimals do not match the mint's decimals");
        return Err(ProgramError::InvalidInstructionData.into());
    }

    let cpi_accounts = TransferChecked {
        mint: ctx.accounts.mint.to_account_info().clone(),
        from: ctx.accounts.sender_ata.to_account_info().clone(),
        to: ctx.accounts.config_pda_ata.to_account_info().clone(),
        authority: ctx.accounts.sender.to_account_info().clone(),
    };
    let cpi_program = ctx.accounts.token_program.to_account_info();
    let cpi_context = CpiContext::new(cpi_program, cpi_accounts)
        .with_remaining_accounts(ctx.remaining_accounts.to_vec());

    token_interface::transfer_checked(cpi_context, gas_fee_amount, decimals)?;

    // Emit an event
    sol_log_data(&[
        event_prefixes::SPL_PAID_FOR_CONTRACT_CALL,
        &ctx.accounts.config_pda.to_account_info().key.to_bytes(),
        &ctx.accounts.config_pda_ata.to_account_info().key.to_bytes(),
        &ctx.accounts.mint.to_account_info().key.to_bytes(),
        &ctx.accounts.token_program.key.to_bytes(),
        &destination_chain.into_bytes(),
        &destination_address.into_bytes(),
        &payload_hash,
        &refund_address.to_bytes(),
        params,
        &gas_fee_amount.to_le_bytes(),
    ]);

    Ok(())
}
