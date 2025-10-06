use crate::events::SplGasRefundedEvent;
use crate::state::Treasury;
use anchor_lang::prelude::*;
use anchor_spl::token_interface::{self, Mint, TokenAccount, TokenInterface, TransferChecked};
use axelar_solana_operators::OperatorAccount;

/// Refund previously collected SPL token fees (operator only).
///
/// Accounts expected:
/// 0. `[signer, read-only]` The `operator` account authorized to collect fees.
/// 1. `[writable]` The `receiver` account where the tokens will be sent.
/// 2. `[writable]` The `treasury` account.
/// 3. `[writable]` The treasury's associated token account for the mint.
/// 4. `[]` The mint account for the SPL token.
/// 5. `[]` The SPL token program.
#[event_cpi]
#[derive(Accounts)]
pub struct RefundSplFees<'info> {
    pub operator: Signer<'info>,

    #[account(
        seeds = [
            OperatorAccount::SEED_PREFIX,
            operator.key().as_ref(),
        ],
        bump = operator_pda.bump,
        seeds::program = axelar_solana_operators::ID
    )]
    pub operator_pda: Account<'info, OperatorAccount>,

    #[account(
        mut,
        token::mint = mint,
    )]
    pub receiver_token_account: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [
            Treasury::SEED_PREFIX,
        ],
        bump = treasury.load()?.bump,
    )]
    pub treasury: AccountLoader<'info, Treasury>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = treasury,
    )]
    pub treasury_token_account: InterfaceAccount<'info, TokenAccount>,

    pub mint: InterfaceAccount<'info, Mint>,

    pub token_program: Interface<'info, TokenInterface>,
}

pub fn refund_spl_fees(
    ctx: Context<RefundSplFees>,
    tx_hash: [u8; 64],
    ix_index: u8,
    event_ix_index: u8,
    fees: u64,
    decimals: u8,
) -> Result<()> {
    if fees == 0 {
        msg!("Gas fee amount cannot be zero");
        return Err(ProgramError::InvalidInstructionData.into());
    }

    let signer_seeds: &[&[&[u8]]] =
        &[&[Treasury::SEED_PREFIX, &[ctx.accounts.treasury.load()?.bump]]];

    let cpi_accounts = TransferChecked {
        mint: ctx.accounts.mint.to_account_info().clone(),
        from: ctx
            .accounts
            .treasury_token_account
            .to_account_info()
            .clone(),
        to: ctx
            .accounts
            .receiver_token_account
            .to_account_info()
            .clone(),
        authority: ctx.accounts.treasury.to_account_info().clone(),
    };
    let cpi_program = ctx.accounts.token_program.to_account_info();
    let cpi_context = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer_seeds);

    token_interface::transfer_checked(cpi_context, fees, decimals)?;

    emit_cpi!(SplGasRefundedEvent {
        treasury_token_account: *ctx.accounts.treasury_token_account.to_account_info().key,
        mint: *ctx.accounts.mint.to_account_info().key,
        token_program_id: *ctx.accounts.token_program.to_account_info().key,
        tx_hash,
        treasury: *ctx.accounts.treasury.to_account_info().key,
        ix_index,
        event_ix_index,
        receiver: *ctx.accounts.receiver_token_account.to_account_info().key,
        fees,
    });

    Ok(())
}
