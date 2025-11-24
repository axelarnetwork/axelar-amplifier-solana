use crate::{
    state::{InterchainTokenService, RoleProposal, Roles, RolesError, TokenManager, UserRoles},
    ItsError,
};
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct AcceptInterchainTokenMintership<'info> {
    /// Payer for transaction fees
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

    /// Destination user account (the one accepting the mintership role)
    pub destination_user_account: Signer<'info>,

    /// Destination user roles account (will receive MINTER role for this token manager)
    #[account(
        init_if_needed,
        payer = payer,
        space = UserRoles::DISCRIMINATOR.len() + UserRoles::INIT_SPACE,
        seeds = [
            UserRoles::SEED_PREFIX,
            token_manager_account.key().as_ref(),
            destination_user_account.key().as_ref(),
        ],
        bump,
    )]
    pub destination_roles_account: Account<'info, UserRoles>,

    /// Origin user account (current minter who proposed the transfer)
    // Note: We shouldn't need this since its checked by the propose instruction
    #[account(
        mut,
        constraint = origin_user_account.key() != destination_user_account.key()
            @ ItsError::InvalidArgument,
    )]
    pub origin_user_account: AccountInfo<'info>,

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

    /// Role proposal PDA for the destination user for this token manager
    #[account(
        mut,
        seeds = [
            RoleProposal::SEED_PREFIX,
            token_manager_account.key().as_ref(),
            origin_user_account.key().as_ref(),
            destination_user_account.key().as_ref(),
        ],
        bump = proposal_account.bump,
        constraint = proposal_account.roles.contains(Roles::MINTER)
            @ RolesError::ProposalMissingMinterRole,
        // Return balance to origin user
        close = origin_user_account,
    )]
    pub proposal_account: Account<'info, RoleProposal>,

    pub system_program: Program<'info, System>,
}

pub fn accept_interchain_token_mintership_handler(
    ctx: Context<AcceptInterchainTokenMintership>,
) -> Result<()> {
    msg!("Instruction: AcceptInterchainTokenMintership");

    let origin_roles = &mut ctx.accounts.origin_roles_account;
    let destination_roles = &mut ctx.accounts.destination_roles_account;

    // Remove MINTER role from origin user
    origin_roles.roles.remove(Roles::MINTER);

    // Add MINTER role to destination user
    destination_roles.roles.insert(Roles::MINTER);
    destination_roles.bump = ctx.bumps.destination_roles_account;

    msg!(
        "Interchain token mintership accepted for token_id {:?}: transferred from {} to {}",
        ctx.accounts.token_manager_account.token_id,
        ctx.accounts.origin_user_account.key(),
        ctx.accounts.destination_user_account.key()
    );

    // Close the origin roles account if no remaining roles
    if !origin_roles.has_roles() {
        anchor_lang::AccountsClose::close(
            &ctx.accounts.origin_roles_account,
            ctx.accounts.payer.to_account_info(),
        )
        .map_err(|e| e.with_account_name("origin_roles_account"))?;
    }

    Ok(())
}
