use crate::{
    errors::ITSError,
    events::DeployRemoteInterchainTokenApproval,
    seed_prefixes::{DEPLOYMENT_APPROVAL_SEED, TOKEN_MANAGER_SEED},
    state::{deploy_approval::DeployApproval, InterchainTokenService, TokenManager, UserRoles},
    utils::interchain_token_id,
};
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[event_cpi]
#[instruction(deployer: Pubkey, salt: [u8; 32], destination_chain: String, destination_minter: Vec<u8>)]
pub struct ApproveDeployRemoteInterchainToken<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    pub minter: Signer<'info>,

    #[account(
        seeds = [
            TOKEN_MANAGER_SEED,
            find_its_root_pda().key().as_ref(),
            &interchain_token_id(&deployer, &salt)
        ],
        bump = token_manager_pda.bump
    )]
    pub token_manager_pda: Account<'info, TokenManager>,

    #[account(
        seeds = [
            UserRoles::SEED_PREFIX,
            token_manager_pda.key().as_ref(),
            minter.key().as_ref()
        ],
        bump = minter_roles.bump,
        constraint = minter_roles.has_minter_role() @ ITSError::InvalidArgument
    )]
    pub minter_roles: Account<'info, UserRoles>,

    #[account(
        init,
        payer = payer,
        space = DeployApproval::DISCRIMINATOR.len() + DeployApproval::INIT_SPACE,
        seeds = [
            DEPLOYMENT_APPROVAL_SEED,
            minter.key().as_ref(),
            &interchain_token_id(&deployer, &salt),
            &anchor_lang::solana_program::keccak::hashv(&[destination_chain.as_bytes()]).to_bytes()
        ],
        bump
    )]
    pub deploy_approval_pda: Account<'info, DeployApproval>,

    pub system_program: Program<'info, System>,
}

pub fn approve_deploy_remote_interchain_token(
    ctx: Context<ApproveDeployRemoteInterchainToken>,
    deployer: Pubkey,
    salt: [u8; 32],
    destination_chain: String,
    destination_minter: Vec<u8>,
) -> Result<()> {
    msg!("Instruction: ApproveDeployRemoteInterchainToken");

    let token_id = interchain_token_id(&deployer, &salt);

    // Initialize the deploy approval account
    let deploy_approval = &mut ctx.accounts.deploy_approval_pda;
    deploy_approval.approved_destination_minter =
        anchor_lang::solana_program::keccak::hash(&destination_minter).to_bytes();
    deploy_approval.bump = ctx.bumps.deploy_approval_pda;

    emit_cpi!(DeployRemoteInterchainTokenApproval {
        minter: ctx.accounts.minter.key(),
        deployer,
        token_id,
        destination_chain,
        destination_minter,
    });

    Ok(())
}

pub fn find_its_root_pda() -> Pubkey {
    let (its_root_pda, _bump) =
        Pubkey::find_program_address(&[InterchainTokenService::SEED_PREFIX], &crate::ID);
    its_root_pda
}
