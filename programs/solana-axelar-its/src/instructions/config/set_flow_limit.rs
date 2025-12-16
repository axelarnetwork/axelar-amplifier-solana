use crate::{
    events::FlowLimitSet,
    state::{roles, InterchainTokenService, RolesError, TokenManager, UserRoles},
    ItsError,
};
use anchor_lang::prelude::*;
use anchor_lang::solana_program::instruction::Instruction;
use anchor_lang::InstructionData;

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
        constraint = its_roles_pda.contains(roles::OPERATOR) @ RolesError::MissingOperatorRole,
    )]
    pub its_roles_pda: Account<'info, UserRoles>,

    #[account(
        mut,
        seeds = [
            TokenManager::SEED_PREFIX,
            its_root_pda.key().as_ref(),
            &token_manager_pda.token_id,
        ],
        bump = token_manager_pda.bump,
        constraint = token_manager_pda.flow_slot.flow_limit != flow_limit @ ItsError::InvalidArgument,
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

/// Creates a SetFlowLimit instruction
pub fn make_set_flow_limit_instruction(
    payer: Pubkey,
    operator: Pubkey,
    token_id: [u8; 32],
    flow_limit: Option<u64>,
) -> (Instruction, crate::accounts::SetFlowLimit) {
    let its_root_pda = InterchainTokenService::find_pda().0;
    let its_roles_pda = UserRoles::find_pda(&its_root_pda, &operator).0;
    let token_manager_pda = TokenManager::find_pda(token_id, its_root_pda).0;

    let (event_authority, _) = Pubkey::find_program_address(&[b"__event_authority"], &crate::ID);

    let accounts = crate::accounts::SetFlowLimit {
        payer,
        operator,
        its_root_pda,
        its_roles_pda,
        token_manager_pda,
        system_program: anchor_lang::system_program::ID,
        event_authority,
        program: crate::ID,
    };

    (
        Instruction {
            program_id: crate::ID,
            accounts: accounts.to_account_metas(None),
            data: crate::instruction::SetFlowLimit { flow_limit }.data(),
        },
        accounts,
    )
}
