use crate::{
    state::{roles, InterchainTokenService, TokenManager, UserRoles},
    ItsError,
};
use anchor_lang::prelude::*;
use anchor_lang::solana_program::instruction::Instruction;
use anchor_lang::InstructionData;

#[derive(Accounts)]
pub struct TransferInterchainTokenMintership<'info> {
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

    /// Sender user account (signer who currently has MINTER role)
    pub sender_user_account: Signer<'info>,

    /// Sender user roles account (current minter's roles for this token manager)
    #[account(
        mut,
        seeds = [
            UserRoles::SEED_PREFIX,
            token_manager_account.key().as_ref(),
            sender_user_account.key().as_ref(),
        ],
        bump = sender_roles_account.bump,
        constraint = sender_roles_account.has_minter_role()
            @ ItsError::MissingMinterRole,
    )]
    pub sender_roles_account: Account<'info, UserRoles>,

    /// CHECK:
    /// Destination user account (will receive MINTER role)
    #[account(
        constraint = destination_user_account.key() != sender_user_account.key()
            @ ItsError::InvalidArgument,
    )]
    pub destination_user_account: AccountInfo<'info>,

    /// Destination user roles account for this token manager
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
        dup,
    )]
    pub destination_roles_account: Account<'info, UserRoles>,

    pub system_program: Program<'info, System>,
}

pub fn transfer_interchain_token_mintership_handler(
    ctx: Context<TransferInterchainTokenMintership>,
) -> Result<()> {
    msg!("Instruction: TransferInterchainTokenMintership");

    let sender_roles = &mut ctx.accounts.sender_roles_account;
    let destination_roles = &mut ctx.accounts.destination_roles_account;

    // Remove MINTER role from sender user
    sender_roles.remove(roles::MINTER);

    // Add MINTER role to destination user
    destination_roles.insert(roles::MINTER);
    destination_roles.bump = ctx.bumps.destination_roles_account;

    msg!(
        "Transferred interchain token mintership for token_id {:?} from {} to {}",
        ctx.accounts.token_manager_account.token_id,
        ctx.accounts.sender_user_account.key(),
        ctx.accounts.destination_user_account.key()
    );

    // Close sender roles account if no remaining roles
    if !sender_roles.has_roles() {
        ctx.accounts
            .sender_roles_account
            .close(ctx.accounts.payer.to_account_info())
            .map_err(|e| e.with_account_name("sender_roles_account"))?;
    }

    Ok(())
}

pub fn make_transfer_interchain_token_mintership_instruction(
    payer: Pubkey,
    sender: Pubkey,
    destination: Pubkey,
    token_id: [u8; 32],
) -> (
    Instruction,
    crate::accounts::TransferInterchainTokenMintership,
) {
    let its_root_pda = InterchainTokenService::find_pda().0;
    let token_manager_account = TokenManager::find_pda(token_id, its_root_pda).0;
    let sender_roles_account = UserRoles::find_pda(&token_manager_account, &sender).0;
    let destination_roles_account = UserRoles::find_pda(&token_manager_account, &destination).0;

    let accounts = crate::accounts::TransferInterchainTokenMintership {
        payer,
        its_root_pda,
        token_manager_account,
        sender_user_account: sender,
        sender_roles_account,
        destination_user_account: destination,
        destination_roles_account,
        system_program: anchor_lang::system_program::ID,
    };

    (
        Instruction {
            program_id: crate::ID,
            accounts: accounts.to_account_metas(None),
            data: crate::instruction::TransferInterchainTokenMintership {}.data(),
        },
        accounts,
    )
}
