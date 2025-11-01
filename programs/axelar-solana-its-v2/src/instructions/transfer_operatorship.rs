use crate::state::{InterchainTokenService, Roles, RolesError, UserRoles};
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct TransferOperatorship<'info> {
    pub system_program: Program<'info, System>,

    /// Payer for transaction fees and account creation
    #[account(mut)]
    pub payer: Signer<'info>,

    /// Origin user account (signer who currently has OPERATOR role)
    pub origin_user_account: Signer<'info>,

    /// Origin user roles account (current operator's roles)
    #[account(
        mut,
        seeds = [
            UserRoles::SEED_PREFIX,
            resource_account.key().as_ref(),
            origin_user_account.key().as_ref(),
        ],
        bump = origin_roles_account.bump,
        constraint = origin_roles_account.roles.contains(Roles::OPERATOR) @ RolesError::MissingOperatorRole,
    )]
    pub origin_roles_account: Account<'info, UserRoles>,

    /// The ITS root PDA (resource account)
    #[account(
        seeds = [InterchainTokenService::SEED_PREFIX],
        bump = resource_account.bump,
    )]
    pub resource_account: Account<'info, InterchainTokenService>,

    /// Destination user account (will receive OPERATOR role)
    #[account(
        constraint = destination_user_account.key() != origin_user_account.key() @ ProgramError::InvalidArgument,
    )]
    pub destination_user_account: AccountInfo<'info>,

    /// Destination user roles account
    #[account(
        init, // todo: switch to init_if_needed
        payer = payer,
        space = UserRoles::DISCRIMINATOR.len() + UserRoles::INIT_SPACE,
        seeds = [
            UserRoles::SEED_PREFIX,
            resource_account.key().as_ref(),
            destination_user_account.key().as_ref(),
        ],
        bump,
    )]
    pub destination_roles_account: Account<'info, UserRoles>,
}

pub fn transfer_operatorship_handler(ctx: Context<TransferOperatorship>) -> Result<()> {
    msg!("Instruction: TransferOperatorship");

    let origin_roles = &mut ctx.accounts.origin_roles_account;
    let destination_roles = &mut ctx.accounts.destination_roles_account;

    origin_roles.roles.remove(Roles::OPERATOR);

    destination_roles.roles.insert(Roles::OPERATOR);
    destination_roles.bump = ctx.bumps.destination_roles_account;

    msg!(
        "Transferred operatorship from {} to {}",
        ctx.accounts.origin_user_account.key(),
        ctx.accounts.destination_user_account.key()
    );

    Ok(())
}
