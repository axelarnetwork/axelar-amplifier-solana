use crate::{
    events::FlowLimitSet,
    state::{InterchainTokenService, RolesError, TokenManager, UserRoles},
};
use anchor_lang::prelude::*;
use anchor_lang::solana_program::instruction::Instruction;
use anchor_lang::InstructionData;

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
        constraint = flow_limiter_roles_pda.has_flow_limiter_role() @ RolesError::MissingFlowLimiterRole,
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

pub fn make_set_token_manager_flow_limit_instruction(
    payer: Pubkey,
    flow_limiter: Pubkey,
    token_id: [u8; 32],
    flow_limit: Option<u64>,
) -> (Instruction, crate::accounts::SetTokenManagerFlowLimit) {
    let its_root_pda = InterchainTokenService::find_pda().0;
    let token_manager_pda = TokenManager::find_pda(token_id, its_root_pda).0;
    let flow_limiter_roles_pda = UserRoles::find_pda(&token_manager_pda, &flow_limiter).0;

    let (event_authority, _) = crate::EVENT_AUTHORITY_AND_BUMP;

    let accounts = crate::accounts::SetTokenManagerFlowLimit {
        payer,
        flow_limiter,
        its_root_pda,
        token_manager_pda,
        flow_limiter_roles_pda,
        system_program: anchor_lang::system_program::ID,
        event_authority,
        program: crate::ID,
    };

    (
        Instruction {
            program_id: crate::ID,
            accounts: accounts.to_account_metas(None),
            data: crate::instruction::SetTokenManagerFlowLimit { flow_limit }.data(),
        },
        accounts,
    )
}
