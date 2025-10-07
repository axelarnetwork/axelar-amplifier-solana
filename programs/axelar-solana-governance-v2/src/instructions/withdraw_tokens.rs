use crate::{GovernanceConfig, GovernanceError};
use anchor_lang::prelude::*;
use axelar_solana_governance::seed_prefixes;

#[derive(Accounts)]
pub struct WithdrawTokens<'info> {
    pub system_program: Program<'info, System>,
    #[account(
        mut,
        seeds = [seed_prefixes::GOVERNANCE_CONFIG],
        bump = governance_config.load()?.bump,
    )]
    pub governance_config: AccountLoader<'info, GovernanceConfig>,
    /// The account that will receive the withdrawn lamports
    /// CHECK: This can be any account that should receive the funds
    #[account(mut)]
    pub receiver: AccountInfo<'info>,
}

pub fn withdraw_tokens_handler(ctx: Context<WithdrawTokens>, amount: u64) -> Result<()> {
    let governance_config = &ctx.accounts.governance_config;
    let receiver = &ctx.accounts.receiver;

    // Check if governance config has sufficient lamports
    let governance_account_info = governance_config.to_account_info();
    let current_lamports = governance_account_info.lamports();

    require!(
        current_lamports >= amount,
        GovernanceError::InsufficientFunds
    );

    // Perform the transfer
    {
        let mut governance_lamports = governance_account_info.try_borrow_mut_lamports()?;
        let mut receiver_lamports = receiver.try_borrow_mut_lamports()?;

        **governance_lamports = governance_lamports
            .checked_sub(amount)
            .ok_or(GovernanceError::ArithmeticOverflow)?;

        **receiver_lamports = receiver_lamports
            .checked_add(amount)
            .ok_or(GovernanceError::ArithmeticOverflow)?;
    }

    msg!(
        "{} lamports were transferred from {}",
        amount,
        governance_config.key()
    );
    msg!("{} lamports were transferred to {}", amount, receiver.key());

    Ok(())
}
