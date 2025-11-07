use crate::{
    events::FlowLimitSet,
    state::{InterchainTokenService, Roles, RolesError, TokenManager, UserRoles},
};
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[event_cpi]
#[instruction(flow_limit: Option<u64>)]
pub struct SetFlowLimit<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    pub operator: Signer<'info>,

    #[account(
        seeds = [InterchainTokenService::SEED_PREFIX],
        bump = its_root_pda.bump,
    )]
    pub its_root_pda: Account<'info, InterchainTokenService>,

    #[account(
        seeds = [
            UserRoles::SEED_PREFIX,
            its_root_pda.key().as_ref(),
            operator.key().as_ref(),
        ],
        bump = its_roles_pda.bump,
        constraint = its_roles_pda.roles.contains(Roles::OPERATOR) @ RolesError::MissingOperatorRole,
    )]
    pub its_roles_pda: Account<'info, UserRoles>,

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

    pub system_program: Program<'info, System>,
}

pub fn set_flow_limit_handler(ctx: Context<SetFlowLimit>, flow_limit: Option<u64>) -> Result<()> {
    msg!("Instruction: SetFlowLimit");

    // Update the flow limit in the token manager
    ctx.accounts.token_manager_pda.flow_slot.flow_limit = flow_limit;

    emit_cpi!({
        FlowLimitSet {
            token_id: ctx.accounts.token_manager_pda.token_id,
            operator: ctx.accounts.operator.key(),
            flow_limit,
        }
    });

    Ok(())
}
