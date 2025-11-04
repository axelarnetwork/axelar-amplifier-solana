use crate::state::{InterchainTokenService, RoleProposal, Roles, RolesError, UserRoles};
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct ProposeOperatorship<'info> {
    pub system_program: Program<'info, System>,

    /// Payer for transaction fees and account creation
    #[account(mut)]
    pub payer: Signer<'info>,

    /// Origin user account (signer who currently has OPERATOR role)
    pub origin_user_account: Signer<'info>,

    /// Origin user roles account (current operator's roles)
    #[account(
        mut,
        seeds = [
            UserRoles::SEED_PREFIX,
            resource_account.key().as_ref(),
            origin_user_account.key().as_ref(),
        ],
        bump = origin_roles_account.bump,
        constraint = origin_roles_account.roles.contains(Roles::OPERATOR) @ RolesError::MissingOperatorRole,
    )]
    pub origin_roles_account: Account<'info, UserRoles>,

    /// The ITS root PDA (resource account)
    #[account(
        seeds = [InterchainTokenService::SEED_PREFIX],
        bump = resource_account.bump,
    )]
    pub resource_account: Account<'info, InterchainTokenService>,

    /// Destination user account (will receive OPERATOR role)
    #[account(
        constraint = destination_user_account.key() != origin_user_account.key() @ ProgramError::InvalidArgument,
    )]
    pub destination_user_account: AccountInfo<'info>,

    /// Role proposal PDA for the destination user
    #[account(
        init,
        payer = payer,
        space = RoleProposal::DISCRIMINATOR.len() + RoleProposal::INIT_SPACE,
        seeds = [
            RoleProposal::SEED_PREFIX,
            resource_account.key().as_ref(),
            origin_user_account.key().as_ref(),
            destination_user_account.key().as_ref(),
        ],
        bump,
    )]
    pub proposal_account: Account<'info, RoleProposal>,
}

pub fn propose_operatorship_handler(ctx: Context<ProposeOperatorship>) -> Result<()> {
    msg!("Instruction: ProposeOperatorship");

    let proposal = &mut ctx.accounts.proposal_account;

    // Initialize the proposal with OPERATOR role
    proposal.roles = Roles::OPERATOR;
    proposal.bump = ctx.bumps.proposal_account;

    msg!(
        "Proposed operatorship transfer from {} to {}",
        ctx.accounts.origin_user_account.key(),
        ctx.accounts.destination_user_account.key()
    );

    Ok(())
}
