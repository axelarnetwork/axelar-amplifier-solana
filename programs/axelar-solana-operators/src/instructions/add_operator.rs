use crate::state::*;
use crate::ErrorCode;
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct AddOperator<'info> {
    #[account(
        mut,
        address = registry.master_operator @ ErrorCode::UnauthorizedMaster
    )]
    pub master_operator: Signer<'info>,

    /// CHECK: The operator pubkey to add
    pub operator_to_add: AccountInfo<'info>,

    #[account(
        mut,
        seeds = [OperatorRegistry::SEED_PREFIX],
        bump = registry.bump,
    )]
    pub registry: Account<'info, OperatorRegistry>,

    #[account(
        init,
        payer = master_operator,
        space = OperatorAccount::DISCRIMINATOR.len() + OperatorAccount::INIT_SPACE,
        seeds = [
            OperatorAccount::SEED_PREFIX,
            operator_to_add.key().as_ref(),
        ],
        bump,
    )]
    pub operator_account: Account<'info, OperatorAccount>,

    pub system_program: Program<'info, System>,
}

pub fn add_operator(ctx: Context<AddOperator>) -> Result<()> {
    let operator_account = &mut ctx.accounts.operator_account;
    let registry = &mut ctx.accounts.registry;

    operator_account.operator = ctx.accounts.operator_to_add.key();
    operator_account.bump = ctx.bumps.operator_account;

    registry.operator_count = registry
        .operator_count
        .checked_add(1)
        .ok_or::<anchor_lang::error::Error>(ProgramError::ArithmeticOverflow.into())?;

    Ok(())
}
