use crate::{
    state::{InterchainTokenService, RoleProposal, Roles, RolesError, UserRoles},
    ItsError,
};
use anchor_lang::prelude::*;
use anchor_lang::solana_program::instruction::Instruction;
use anchor_lang::InstructionData;

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
        constraint = destination_user_account.key() != origin_user_account.key() @ ItsError::InvalidArgument,
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

/// Creates a ProposeOperatorship instruction
pub fn make_propose_operatorship_instruction(
    payer: Pubkey,
    origin_user_account: Pubkey,
    destination_user_account: Pubkey,
) -> (Instruction, crate::accounts::ProposeOperatorship) {
    let resource_account = InterchainTokenService::find_pda().0;

    let origin_roles_account = UserRoles::find_pda(&resource_account, &origin_user_account).0;
    let (proposal_account, _) = RoleProposal::find_pda(
        &resource_account,
        &origin_user_account,
        &destination_user_account,
        &crate::ID,
    );

    let accounts = crate::accounts::ProposeOperatorship {
        system_program: anchor_lang::system_program::ID,
        payer,
        origin_user_account,
        origin_roles_account,
        resource_account,
        destination_user_account,
        proposal_account,
    };

    (
        Instruction {
            program_id: crate::ID,
            accounts: accounts.to_account_metas(None),
            data: crate::instruction::ProposeOperatorship {}.data(),
        },
        accounts,
    )
}
