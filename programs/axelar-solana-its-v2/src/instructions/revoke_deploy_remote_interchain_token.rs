use crate::{
    events::RevokeDeployRemoteInterchainTokenApproval, seed_prefixes::DEPLOYMENT_APPROVAL_SEED,
    state::deploy_approval::DeployApproval, utils::interchain_token_id,
};
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[event_cpi]
#[instruction(deployer: Pubkey, salt: [u8; 32], destination_chain: String)]
pub struct RevokeDeployRemoteInterchainToken<'info> {
    /// Payer for the transaction (will receive the rent refund)
    #[account(mut)]
    pub payer: Signer<'info>,

    /// The minter who is revoking the deployment approval (must be a signer with minter role)
    pub minter: Signer<'info>,

    /// Deploy approval PDA to be revoked (closed)
    #[account(
        mut,
        close = payer,
        seeds = [
            DEPLOYMENT_APPROVAL_SEED,
            minter.key().as_ref(),
            &interchain_token_id(&deployer, &salt),
            &anchor_lang::solana_program::keccak::hashv(&[destination_chain.as_bytes()]).to_bytes()
        ],
        bump = deploy_approval_pda.bump,
    )]
    pub deploy_approval_pda: Account<'info, DeployApproval>,

    /// System program
    pub system_program: Program<'info, System>,
}

pub fn revoke_deploy_remote_interchain_token(
    ctx: Context<RevokeDeployRemoteInterchainToken>,
    deployer: Pubkey,
    salt: [u8; 32],
    destination_chain: String,
) -> Result<()> {
    msg!("Instruction: RevokeDeployRemoteInterchainToken");

    let token_id = interchain_token_id(&deployer, &salt);

    emit_cpi!(RevokeDeployRemoteInterchainTokenApproval {
        minter: ctx.accounts.minter.key(),
        deployer,
        token_id,
        destination_chain,
    });

    Ok(())
}
