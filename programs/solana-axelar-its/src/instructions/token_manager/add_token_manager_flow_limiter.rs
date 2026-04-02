use crate::{
    state::{roles, InterchainTokenService, RolesError, TokenManager, UserRoles},
    ItsError,
};
use anchor_lang::prelude::*;
use anchor_lang::solana_program::instruction::Instruction;
use anchor_lang::InstructionData;

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
        constraint = authority_roles_account.has_operator_role() @ RolesError::MissingOperatorRole,
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
        bump = token_manager_pda.bump,
    )]
    pub token_manager_pda: Account<'info, TokenManager>,

    /// CHECK:
    /// The user account that will receive the FLOW_LIMITER role
    pub target_user_account: AccountInfo<'info>,

    /// Target user roles account
    #[account(
        init_if_needed,
        payer = payer,
        space = UserRoles::DISCRIMINATOR.len() + UserRoles::INIT_SPACE,
        seeds = [
            UserRoles::SEED_PREFIX,
            token_manager_pda.key().as_ref(),
            target_user_account.key().as_ref(),
        ],
        bump,
        constraint = !target_roles_account.has_flow_limiter_role() @ ItsError::InvalidArgument,
    )]
    pub target_roles_account: Account<'info, UserRoles>,
}

pub fn add_token_manager_flow_limiter_handler(
    ctx: Context<AddTokenManagerFlowLimiter>,
) -> Result<()> {
    msg!("Instruction: AddTokenManagerFlowLimiter");

    let target_roles = &mut ctx.accounts.target_roles_account;

    target_roles.insert(roles::FLOW_LIMITER);
    target_roles.bump = ctx.bumps.target_roles_account;

    Ok(())
}

pub fn make_add_token_manager_flow_limiter_instruction(
    payer: Pubkey,
    authority: Pubkey,
    target: Pubkey,
    token_id: [u8; 32],
) -> (Instruction, crate::accounts::AddTokenManagerFlowLimiter) {
    let its_root_pda = InterchainTokenService::find_pda().0;
    let token_manager_pda = TokenManager::find_pda(token_id, its_root_pda).0;
    let authority_roles_account = UserRoles::find_pda(&token_manager_pda, &authority).0;
    let target_roles_account = UserRoles::find_pda(&token_manager_pda, &target).0;

    let accounts = crate::accounts::AddTokenManagerFlowLimiter {
        system_program: anchor_lang::system_program::ID,
        payer,
        authority_user_account: authority,
        authority_roles_account,
        its_root_pda,
        token_manager_pda,
        target_user_account: target,
        target_roles_account,
    };

    (
        Instruction {
            program_id: crate::ID,
            accounts: accounts.to_account_metas(None),
            data: crate::instruction::AddTokenManagerFlowLimiter {}.data(),
        },
        accounts,
    )
}
