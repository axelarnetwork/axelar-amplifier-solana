use crate::state::{
    InterchainTokenService, RoleProposal, Roles, RolesError, TokenManager, UserRoles,
};
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct AcceptTokenManagerOperatorship<'info> {
    pub system_program: Program<'info, System>,

    /// Payer for transaction fees
    #[account(mut)]
    pub payer: Signer<'info>,

    /// Destination user account (the one accepting the operatorship role)
    pub destination_user_account: Signer<'info>,

    /// Destination user roles account (will receive OPERATOR role for this token manager)
    #[account(
        init_if_needed,
        payer = payer,
        space = UserRoles::DISCRIMINATOR.len() + UserRoles::INIT_SPACE,
        seeds = [
            UserRoles::SEED_PREFIX,
            token_manager_account.key().as_ref(),
            destination_user_account.key().as_ref(),
        ],
        bump,
    )]
    pub destination_roles_account: Account<'info, UserRoles>,

    /// The ITS root PDA
    #[account(
        seeds = [InterchainTokenService::SEED_PREFIX],
        bump = its_root_pda.bump,
    )]
    pub its_root_pda: Account<'info, InterchainTokenService>,

    /// The TokenManager account (resource account for this operation)
    #[account(
        seeds = [
            TokenManager::SEED_PREFIX,
            its_root_pda.key().as_ref(),
            &token_manager_account.token_id,
        ],
        bump = token_manager_account.bump,
    )]
    pub token_manager_account: Account<'info, TokenManager>,

    /// Origin user account (current operator who proposed the transfer)
    #[account(
        mut,
        constraint = origin_user_account.key() != destination_user_account.key() @ ProgramError::InvalidArgument,
    )]
    pub origin_user_account: AccountInfo<'info>,

    /// Origin user roles account (current operator's roles for this token manager)
    #[account(
        mut,
        seeds = [
            UserRoles::SEED_PREFIX,
            token_manager_account.key().as_ref(),
            origin_user_account.key().as_ref(),
        ],
        bump = origin_roles_account.bump,
        constraint = origin_roles_account.roles.contains(Roles::OPERATOR) @ RolesError::MissingOperatorRole,
    )]
    pub origin_roles_account: Account<'info, UserRoles>,

    /// Role proposal PDA for the destination user for this token manager
    #[account(
        mut,
        seeds = [
            RoleProposal::SEED_PREFIX,
            token_manager_account.key().as_ref(),
            origin_user_account.key().as_ref(),
            destination_user_account.key().as_ref(),
        ],
        bump = proposal_account.bump,
        constraint = proposal_account.roles.contains(Roles::OPERATOR) @ RolesError::MissingOperatorRole,
        close = origin_user_account,
    )]
    pub proposal_account: Account<'info, RoleProposal>,
}

pub fn accept_token_manager_operatorship_handler(
    ctx: Context<AcceptTokenManagerOperatorship>,
) -> Result<()> {
    msg!("Instruction: AcceptTokenManagerOperatorship");

    let origin_roles = &mut ctx.accounts.origin_roles_account;
    let destination_roles = &mut ctx.accounts.destination_roles_account;

    // Remove OPERATOR role from origin user
    origin_roles.roles.remove(Roles::OPERATOR);

    // Add OPERATOR role to destination user
    destination_roles.roles.insert(Roles::OPERATOR);
    destination_roles.bump = ctx.bumps.destination_roles_account;

    msg!(
        "Token manager operatorship accepted for token_id {:?}: transferred from {} to {}",
        ctx.accounts.token_manager_account.token_id,
        ctx.accounts.origin_user_account.key(),
        ctx.accounts.destination_user_account.key()
    );

    // Close if no remaining roles
    if !origin_roles.has_roles() {
        anchor_lang::AccountsClose::close(
            &ctx.accounts.origin_roles_account,
            ctx.accounts.payer.to_account_info(),
        )
        .map_err(|e| e.with_account_name("origin_roles_account"))?;
    }

    Ok(())
}
