use crate::state::{InterchainTokenService, Roles, RolesError, TokenManager, UserRoles};
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct AddTokenManagerFlowLimiter<'info> {
    pub system_program: Program<'info, System>,

    /// Payer for transaction fees and account creation
    #[account(mut)]
    pub payer: Signer<'info>,

    /// The authority (operator) who is adding the flow limiter role
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

    /// The token manager for which flow limiter role is being added
    #[account(
        seeds = [
            crate::seed_prefixes::TOKEN_MANAGER_SEED,
            its_root_pda.key().as_ref(),
            &token_manager_pda.token_id
        ],
        seeds::program = crate::ID,
        bump = token_manager_pda.bump,
    )]
    pub token_manager_pda: Account<'info, TokenManager>,

    /// The user account that will receive the FLOW_LIMITER role
    #[account(
        constraint = target_user_account.key() != authority_user_account.key() @ ProgramError::InvalidArgument,
    )]
    pub target_user_account: AccountInfo<'info>,

    /// Target user roles account
    #[account(
        init, // todo: switch to init_if_needed
        payer = payer,
        space = UserRoles::DISCRIMINATOR.len() + UserRoles::INIT_SPACE,
        seeds = [
            UserRoles::SEED_PREFIX,
            token_manager_pda.key().as_ref(),
            target_user_account.key().as_ref(),
        ],
        bump,
    )]
    pub target_roles_account: Account<'info, UserRoles>,
}

pub fn add_token_manager_flow_limiter_handler(
    ctx: Context<AddTokenManagerFlowLimiter>,
) -> Result<()> {
    msg!("Instruction: AddTokenManagerFlowLimiter");

    let target_roles = &mut ctx.accounts.target_roles_account;

    // Add the FLOW_LIMITER role
    target_roles.roles.insert(Roles::FLOW_LIMITER);
    target_roles.bump = ctx.bumps.target_roles_account;

    msg!(
        "Added FLOW_LIMITER role for token_id: {:?}, user: {}",
        ctx.accounts.token_manager_pda.token_id,
        ctx.accounts.target_user_account.key()
    );

    Ok(())
}
