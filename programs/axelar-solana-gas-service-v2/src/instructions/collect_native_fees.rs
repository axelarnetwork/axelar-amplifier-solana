use crate::state::Treasury;
use anchor_lang::prelude::*;
use axelar_solana_operators::OperatorAccount;

/// Collect accrued native SOL fees (operator only).
///
/// Accounts expected:
/// 1. `[signer, read-only]` The `operator` account authorized to collect fees.
/// 2. `[writable]` The `config_pda` account holding the accrued lamports to collect.
/// 3. `[writable]` The `receiver` account where the collected lamports will be sent.
#[derive(Accounts)]
pub struct CollectNativeFees<'info> {
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
        seeds = [
            Treasury::SEED_PREFIX,
        ],
        bump = treasury.bump,
    )]
    pub treasury: Account<'info, Treasury>,

    /// CHECK: Can be any account to receive funds
    #[account(mut)]
    pub receiver: UncheckedAccount<'info>,
}

pub fn collect_native_fees(ctx: Context<CollectNativeFees>, amount: u64) -> Result<()> {
    if amount == 0 {
        msg!("Gas fee amount cannot be zero");
        return Err(ProgramError::InvalidInstructionData.into());
    }

    // TODO(v2) consider making this a utility function in program-utils
    // similar to transfer_lamports
    if ctx.accounts.treasury.get_lamports() < amount {
        return Err(ProgramError::InsufficientFunds.into());
    }
    ctx.accounts.treasury.sub_lamports(amount)?;
    ctx.accounts.receiver.add_lamports(amount)?;

    Ok(())
}
