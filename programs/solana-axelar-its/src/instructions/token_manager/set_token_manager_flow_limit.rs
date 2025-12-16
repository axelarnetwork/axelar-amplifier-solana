use crate::{
    events::FlowLimitSet,
    state::{roles, InterchainTokenService, RolesError, TokenManager, UserRoles},
};
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[event_cpi]
#[instruction(flow_limit: Option<u64>)]
pub struct SetTokenManagerFlowLimit<'info> {
    /// Payer for transaction fees
    #[account(mut)]
    pub payer: Signer<'info>,

    /// The flow limiter who can set the flow limit for this specific token manager
    pub flow_limiter: Signer<'info>,

    /// The ITS root PDA
    #[account(
        seeds = [InterchainTokenService::SEED_PREFIX],
        bump = its_root_pda.bump,
    )]
    pub its_root_pda: Account<'info, InterchainTokenService>,

    /// The token manager for which the flow limit is being set
    #[account(
        mut,
        seeds = [
            crate::seed_prefixes::TOKEN_MANAGER_SEED,
            its_root_pda.key().as_ref(),
            &token_manager_pda.token_id
        ],
        bump = token_manager_pda.bump,
    )]
    pub token_manager_pda: Account<'info, TokenManager>,

    /// Flow limiter's roles account on the token manager (must have FLOW_LIMITER role)
    #[account(
        seeds = [
            UserRoles::SEED_PREFIX,
            token_manager_pda.key().as_ref(),
            flow_limiter.key().as_ref(),
        ],
        bump = flow_limiter_roles_pda.bump,
        constraint = flow_limiter_roles_pda.contains(roles::FLOW_LIMITER) @ RolesError::MissingFlowLimiterRole,
    )]
    pub flow_limiter_roles_pda: Account<'info, UserRoles>,

    pub system_program: Program<'info, System>,
}

pub fn set_token_manager_flow_limit_handler(
    ctx: Context<SetTokenManagerFlowLimit>,
    flow_limit: Option<u64>,
) -> Result<()> {
    msg!("Instruction: SetTokenManagerFlowLimit");

    // Update the flow limit in the token manager
    ctx.accounts.token_manager_pda.flow_slot.flow_limit = flow_limit;

    emit_cpi!({
        FlowLimitSet {
            token_id: ctx.accounts.token_manager_pda.token_id,
            operator: ctx.accounts.flow_limiter.key(),
            flow_limit,
        }
    });

    Ok(())
}
