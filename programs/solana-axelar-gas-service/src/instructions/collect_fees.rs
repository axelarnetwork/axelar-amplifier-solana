use crate::{events::GasCollectedEvent, state::Treasury};
use anchor_lang::prelude::*;
use solana_axelar_operators::OperatorAccount;
use solana_axelar_std::transfer_lamports_anchor;

/// Collect accrued native SOL fees (operator only).
#[derive(Accounts)]
#[event_cpi]
pub struct CollectFees<'info> {
    pub operator: Signer<'info>,

    #[account(
        seeds = [
            OperatorAccount::SEED_PREFIX,
            operator.key().as_ref(),
        ],
        bump = operator_pda.bump,
        seeds::program = solana_axelar_operators::ID
    )]
    pub operator_pda: Account<'info, OperatorAccount>,

    #[account(
        mut,
        seeds = [
            Treasury::SEED_PREFIX,
        ],
        bump = treasury.load()?.bump,
    )]
    pub treasury: AccountLoader<'info, Treasury>,

    /// CHECK: Can be any account to receive funds
    #[account(mut)]
    pub receiver: UncheckedAccount<'info>,
}

pub fn collect_native_fees(ctx: Context<CollectFees>, amount: u64) -> Result<()> {
    if amount == 0 {
        msg!("Gas fee amount cannot be zero");
        return Err(ProgramError::InvalidInstructionData.into());
    }

    transfer_lamports_anchor!(
        ctx.accounts.treasury.to_account_info(),
        ctx.accounts.receiver.to_account_info(),
        amount
    );

    if !Rent::get()?.is_exempt(
        ctx.accounts.treasury.get_lamports(),
        Treasury::INIT_SPACE + Treasury::DISCRIMINATOR.len(),
    ) {
        msg!("Treasury account is not rent exempt after fee collection");
        return Err(ProgramError::InvalidAccountData.into());
    }

    emit_cpi!(GasCollectedEvent {
        receiver: ctx.accounts.receiver.key(),
        amount,
        spl_token_account: None,
    });

    Ok(())
}
