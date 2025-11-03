use crate::state::{InterchainTokenService, Roles, RolesError, TokenManager, UserRoles};
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct RemoveTokenManagerFlowLimiter<'info> {
    pub system_program: Program<'info, System>,

    /// Payer for transaction fees and account rent refunds
    #[account(mut)]
    pub payer: Signer<'info>,

    /// The authority (operator) who is removing the flow limiter role
    pub authority_user_account: Signer<'info>,

    /// Authority user roles account (must have OPERATOR role)
    #[account(
        seeds = [
            UserRoles::SEED_PREFIX,
            token_manager_pda.key().as_ref(),
            authority_user_account.key().as_ref(),
        ],
        bump = authority_roles_account.bump,
        constraint = authority_roles_account.roles.contains(Roles::OPERATOR) @ RolesError::MissingOperatorRole,
    )]
    pub authority_roles_account: Account<'info, UserRoles>,

    /// The ITS root PDA
    #[account(
        seeds = [InterchainTokenService::SEED_PREFIX],
        bump = its_root_pda.bump,
    )]
    pub its_root_pda: Account<'info, InterchainTokenService>,

    /// The token manager from which flow limiter role is being removed
    #[account(
        seeds = [
            crate::seed_prefixes::TOKEN_MANAGER_SEED,
            its_root_pda.key().as_ref(),
            &token_manager_pda.token_id
        ],
        bump = token_manager_pda.bump,
    )]
    pub token_manager_pda: Account<'info, TokenManager>,

    /// The user account from which the FLOW_LIMITER role will be removed
    #[account(
        constraint = target_user_account.key() != authority_user_account.key() @ ProgramError::InvalidArgument,
    )]
    pub target_user_account: AccountInfo<'info>,

    /// Target user roles account (must exist and have FLOW_LIMITER role)
    // todo: should we close this if no roles remain?
    #[account(
        mut,
        seeds = [
            UserRoles::SEED_PREFIX,
            token_manager_pda.key().as_ref(),
            target_user_account.key().as_ref(),
        ],
        bump = target_roles_account.bump,
        constraint = target_roles_account.roles.contains(Roles::FLOW_LIMITER) @ RolesError::MissingFlowLimiterRole,
    )]
    pub target_roles_account: Account<'info, UserRoles>,
}

pub fn remove_token_manager_flow_limiter_handler(
    ctx: Context<RemoveTokenManagerFlowLimiter>,
) -> Result<()> {
    msg!("Instruction: RemoveTokenManagerFlowLimiter");

    let target_roles = &mut ctx.accounts.target_roles_account;

    // Remove the FLOW_LIMITER role
    target_roles.roles.remove(Roles::FLOW_LIMITER);

    msg!(
        "Removed FLOW_LIMITER role for token_id: {:?}, user: {}",
        ctx.accounts.token_manager_pda.token_id,
        ctx.accounts.target_user_account.key()
    );

    Ok(())
}
