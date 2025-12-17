use crate::{
    state::{roles, InterchainTokenService, RolesError, UserRoles},
    ItsError,
};
use anchor_lang::solana_program::instruction::Instruction;
use anchor_lang::{prelude::*, InstructionData};

#[derive(Accounts)]
pub struct TransferOperatorship<'info> {
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
        constraint = origin_roles_account.contains(roles::OPERATOR) @ RolesError::MissingOperatorRole,
    )]
    pub origin_roles_account: Account<'info, UserRoles>,

    /// The ITS root PDA (resource account)
    #[account(
        seeds = [InterchainTokenService::SEED_PREFIX],
        bump = resource_account.bump,
    )]
    pub resource_account: Account<'info, InterchainTokenService>,

    /// CHECK:
    /// Destination user account (will receive OPERATOR role)
    #[account(
        constraint = destination_user_account.key() != origin_user_account.key() @ ItsError::InvalidArgument,
    )]
    pub destination_user_account: AccountInfo<'info>,

    /// Destination user roles account
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
}

pub fn transfer_operatorship_handler(ctx: Context<TransferOperatorship>) -> Result<()> {
    msg!("Instruction: TransferOperatorship");

    let origin_roles = &mut ctx.accounts.origin_roles_account;
    let destination_roles = &mut ctx.accounts.destination_roles_account;

    origin_roles.remove(roles::OPERATOR);

    destination_roles.insert(roles::OPERATOR);
    destination_roles.bump = ctx.bumps.destination_roles_account;

    msg!(
        "Transferred operatorship from {} to {}",
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

/// Creates a TransferOperatorship instruction
pub fn make_transfer_operatorship_instruction(
    payer: Pubkey,
    origin_user_account: Pubkey,
    destination_user_account: Pubkey,
) -> (Instruction, crate::accounts::TransferOperatorship) {
    let resource_account = InterchainTokenService::find_pda().0;

    let origin_roles_account = UserRoles::find_pda(&resource_account, &origin_user_account).0;
    let destination_roles_account =
        UserRoles::find_pda(&resource_account, &destination_user_account).0;

    let accounts = crate::accounts::TransferOperatorship {
        system_program: anchor_lang::system_program::ID,
        payer,
        origin_user_account,
        origin_roles_account,
        resource_account,
        destination_user_account,
        destination_roles_account,
    };

    (
        Instruction {
            program_id: crate::ID,
            accounts: accounts.to_account_metas(None),
            data: crate::instruction::TransferOperatorship {}.data(),
        },
        accounts,
    )
}
