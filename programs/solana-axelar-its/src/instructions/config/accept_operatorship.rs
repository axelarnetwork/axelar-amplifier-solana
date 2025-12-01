use crate::{
    state::{InterchainTokenService, RoleProposal, Roles, RolesError, UserRoles},
    ItsError,
};
use anchor_lang::prelude::*;
use anchor_lang::solana_program::instruction::Instruction;
use anchor_lang::InstructionData;

#[derive(Accounts)]
pub struct AcceptOperatorship<'info> {
    pub system_program: Program<'info, System>,

    /// Payer for transaction fees
    #[account(mut)]
    pub payer: Signer<'info>,

    /// Destination user account (the one accepting the operatorship role)
    pub destination_user_account: Signer<'info>,

    /// Destination user roles account (will receive OPERATOR role)
    #[account(
        init_if_needed,
        payer = payer,
        space = UserRoles::DISCRIMINATOR.len() + UserRoles::INIT_SPACE,
        seeds = [
            UserRoles::SEED_PREFIX,
            resource_account.key().as_ref(),
            destination_user_account.key().as_ref(),
        ],
        bump,
    )]
    pub destination_roles_account: Account<'info, UserRoles>,

    /// The ITS root PDA (resource account)
    #[account(
        seeds = [InterchainTokenService::SEED_PREFIX],
        bump = resource_account.bump,
    )]
    pub resource_account: Account<'info, InterchainTokenService>,

    /// Origin user account (current operator who proposed the transfer)
    #[account(
        mut,
        constraint = origin_user_account.key() != destination_user_account.key()
            @ ItsError::InvalidArgument,
    )]
    pub origin_user_account: AccountInfo<'info>,

    /// Origin user roles account (current operator's roles)
    #[account(
        mut,
        seeds = [
            UserRoles::SEED_PREFIX,
            resource_account.key().as_ref(),
            origin_user_account.key().as_ref(),
        ],
        bump = origin_roles_account.bump,
        constraint = origin_roles_account.roles.contains(Roles::OPERATOR)
            @ RolesError::MissingOperatorRole,
    )]
    pub origin_roles_account: Account<'info, UserRoles>,

    /// Role proposal PDA for the destination user
    #[account(
        mut,
        seeds = [
            RoleProposal::SEED_PREFIX,
            resource_account.key().as_ref(),
            origin_user_account.key().as_ref(),
            destination_user_account.key().as_ref(),
        ],
        bump = proposal_account.bump,
        constraint = proposal_account.roles.contains(Roles::OPERATOR)
            @ RolesError::ProposalMissingOperatorRole,
        // Return balance to origin user (proposer)
        close = origin_user_account,
    )]
    pub proposal_account: Account<'info, RoleProposal>,
}

pub fn accept_operatorship_handler(ctx: Context<AcceptOperatorship>) -> Result<()> {
    msg!("Instruction: AcceptOperatorship");

    let origin_roles = &mut ctx.accounts.origin_roles_account;
    let destination_roles = &mut ctx.accounts.destination_roles_account;

    origin_roles.roles.remove(Roles::OPERATOR);

    destination_roles.roles.insert(Roles::OPERATOR);
    destination_roles.bump = ctx.bumps.destination_roles_account;

    msg!(
        "Operatorship accepted: transferred from {} to {}",
        ctx.accounts.origin_user_account.key(),
        ctx.accounts.destination_user_account.key()
    );

    // Close if no remaining roles
    if !origin_roles.has_roles() {
        ctx.accounts
            .origin_roles_account
            .close(ctx.accounts.payer.to_account_info())
            .map_err(|e| e.with_account_name("origin_roles_account"))?;
    }

    Ok(())
}

/// Creates an AcceptOperatorship instruction
pub fn make_accept_operatorship_instruction(
    payer: Pubkey,
    origin_user_account: Pubkey,
    destination_user_account: Pubkey,
) -> (Instruction, crate::accounts::AcceptOperatorship) {
    let resource_account = InterchainTokenService::find_pda().0;

    let origin_roles_account = UserRoles::find_pda(&resource_account, &origin_user_account).0;
    let destination_roles_account =
        UserRoles::find_pda(&resource_account, &destination_user_account).0;
    let (proposal_account, _) = RoleProposal::find_pda(
        &resource_account,
        &origin_user_account,
        &destination_user_account,
        &crate::ID,
    );

    let accounts = crate::accounts::AcceptOperatorship {
        system_program: anchor_lang::system_program::ID,
        payer,
        destination_user_account,
        destination_roles_account,
        resource_account,
        origin_user_account,
        origin_roles_account,
        proposal_account,
    };

    (
        Instruction {
            program_id: crate::ID,
            accounts: accounts.to_account_metas(None),
            data: crate::instruction::AcceptOperatorship {}.data(),
        },
        accounts,
    )
}
