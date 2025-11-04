use crate::state::{InterchainTokenService, RoleProposal, Roles, RolesError, UserRoles};
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct AcceptOperatorship<'info> {
    pub system_program: Program<'info, System>,

    /// Payer for transaction fees
    #[account(mut)]
    pub payer: Signer<'info>,

    /// Destination user account (the one accepting the operatorship role)
    pub destination_user_account: Signer<'info>,

    /// Destination user roles account (will receive OPERATOR role)
    #[account(
        init_if_needed,
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

    /// The ITS root PDA (resource account)
    #[account(
        seeds = [InterchainTokenService::SEED_PREFIX],
        bump = resource_account.bump,
    )]
    pub resource_account: Account<'info, InterchainTokenService>,

    /// Origin user account (current operator who proposed the transfer)
    #[account(
        mut,
        constraint = origin_user_account.key() != destination_user_account.key() @ ProgramError::InvalidArgument,
    )]
    pub origin_user_account: AccountInfo<'info>,

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

    /// Role proposal PDA for the destination user
    #[account(
        mut,
        seeds = [
            RoleProposal::SEED_PREFIX,
            resource_account.key().as_ref(),
            origin_user_account.key().as_ref(),
            destination_user_account.key().as_ref(),
        ],
        bump = proposal_account.bump,
        constraint = proposal_account.roles.contains(Roles::OPERATOR) @ RolesError::MissingOperatorRole,
        close = origin_user_account,
    )]
    pub proposal_account: Account<'info, RoleProposal>,
}

pub fn accept_operatorship_handler(ctx: Context<AcceptOperatorship>) -> Result<()> {
    msg!("Instruction: AcceptOperatorship");

    let origin_roles = &mut ctx.accounts.origin_roles_account;
    let destination_roles = &mut ctx.accounts.destination_roles_account;

    origin_roles.roles.remove(Roles::OPERATOR);

    destination_roles.roles.insert(Roles::OPERATOR);
    destination_roles.bump = ctx.bumps.destination_roles_account;

    msg!(
        "Operatorship accepted: transferred from {} to {}",
        ctx.accounts.origin_user_account.key(),
        ctx.accounts.destination_user_account.key()
    );

    Ok(())
}
