use crate::{
    state::{InterchainTokenService, Roles, RolesError, TokenManager, UserRoles},
    ItsError,
};
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct TransferTokenManagerOperatorship<'info> {
    pub system_program: Program<'info, System>,

    /// Payer for transaction fees and account creation
    #[account(mut)]
    pub payer: Signer<'info>,

    /// Origin user account (signer who currently has OPERATOR role)
    pub origin_user_account: Signer<'info>,

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

    /// Destination user account (will receive OPERATOR role)
    #[account(
        constraint = destination_user_account.key() != origin_user_account.key() @ ItsError::InvalidArgument,
    )]
    pub destination_user_account: AccountInfo<'info>,

    /// Destination user roles account for this token manager
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
}

pub fn transfer_token_manager_operatorship_handler(
    ctx: Context<TransferTokenManagerOperatorship>,
) -> Result<()> {
    msg!("Instruction: TransferTokenManagerOperatorship");

    let origin_roles = &mut ctx.accounts.origin_roles_account;
    let destination_roles = &mut ctx.accounts.destination_roles_account;

    // Remove OPERATOR role from origin user
    origin_roles.roles.remove(Roles::OPERATOR);

    // Add OPERATOR role to destination user
    destination_roles.roles.insert(Roles::OPERATOR);
    destination_roles.bump = ctx.bumps.destination_roles_account;

    msg!(
        "Transferred token manager operatorship for token_id {:?} from {} to {}",
        ctx.accounts.token_manager_account.token_id,
        ctx.accounts.origin_user_account.key(),
        ctx.accounts.destination_user_account.key()
    );

    // Close if no remaining roles
    if !origin_roles.has_roles() {
        ctx.accounts
            .origin_roles_account
            .close(ctx.accounts.payer.to_account_info())
            .map_err(|e| e.with_account_name("origin_roles_account"))?;
    }

    Ok(())
}
