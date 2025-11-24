use crate::{
    state::{InterchainTokenService, RoleProposal, Roles, RolesError, TokenManager, UserRoles},
    ItsError,
};
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct ProposeInterchainTokenMintership<'info> {
    /// Payer for transaction fees and account creation
    #[account(mut)]
    pub payer: Signer<'info>,

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

    /// Origin user account (signer who currently has MINTER role)
    pub origin_user_account: Signer<'info>,

    /// Origin user roles account (current minter's roles for this token manager)
    #[account(
        mut,
        seeds = [
            UserRoles::SEED_PREFIX,
            token_manager_account.key().as_ref(),
            origin_user_account.key().as_ref(),
        ],
        bump = origin_roles_account.bump,
        constraint = origin_roles_account.roles.contains(Roles::MINTER)
            @ RolesError::MissingMinterRole,
    )]
    pub origin_roles_account: Account<'info, UserRoles>,

    /// Destination user account (will receive MINTER role)
    #[account(
        constraint = destination_user_account.key() != origin_user_account.key()
            @ ItsError::InvalidArgument,
    )]
    pub destination_user_account: AccountInfo<'info>,

    #[account(
        init,
        payer = payer,
        space = RoleProposal::DISCRIMINATOR.len() + RoleProposal::INIT_SPACE,
        seeds = [
            RoleProposal::SEED_PREFIX,
            token_manager_account.key().as_ref(),
            origin_user_account.key().as_ref(),
            destination_user_account.key().as_ref(),
        ],
        bump,
    )]
    pub proposal_account: Account<'info, RoleProposal>,

    pub system_program: Program<'info, System>,
}

pub fn propose_token_manager_mintership_handler(
    ctx: Context<ProposeInterchainTokenMintership>,
) -> Result<()> {
    msg!("Instruction: ProposeTokenManagerMintership");

    let proposal = &mut ctx.accounts.proposal_account;

    // Initialize the proposal with MINTER role
    proposal.roles = Roles::MINTER;
    proposal.bump = ctx.bumps.proposal_account;

    msg!(
        "Proposed token manager mintership transfer for token_id {:?} from {} to {}",
        ctx.accounts.token_manager_account.token_id,
        ctx.accounts.origin_user_account.key(),
        ctx.accounts.destination_user_account.key()
    );

    Ok(())
}
