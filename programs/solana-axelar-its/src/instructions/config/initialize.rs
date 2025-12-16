#![allow(clippy::missing_asserts_for_indexing)]
use crate::{
    state::{InterchainTokenService, Roles, UserRoles},
    utils::relayer_transaction,
    ItsError,
};
use anchor_lang::prelude::*;
#[allow(deprecated)]
use anchor_lang::solana_program::bpf_loader_upgradeable;
use anchor_lang::solana_program::instruction::Instruction;
use anchor_lang::InstructionData;
use relayer_discovery::TRANSACTION_PDA_SEED;

/// Initialize the configuration PDA.
#[derive(Accounts)]
#[instruction(chain_name: String, its_hub_address: String)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        seeds = [crate::ID.as_ref()],
        bump,
        seeds::program = bpf_loader_upgradeable::ID,
        constraint = program_data.upgrade_authority_address == Some(payer.key())
            @ ItsError::InvalidAccountData
    )]
    pub program_data: Account<'info, ProgramData>,

    #[account(
        init,
        payer = payer,
        space = InterchainTokenService::space_for(its_hub_address.len(), chain_name.len(), 0),
        seeds = [InterchainTokenService::SEED_PREFIX],
        bump,
    )]
    pub its_root_pda: Account<'info, InterchainTokenService>,

    pub system_program: Program<'info, System>,

    pub operator: Signer<'info>,

    /// The address of the account that will store the roles of the operator account.
    #[account(
    	init,
	 	payer = payer,
	 	space = UserRoles::DISCRIMINATOR.len() + UserRoles::INIT_SPACE,
	 	seeds = [
			UserRoles::SEED_PREFIX,
			its_root_pda.key().as_ref(),
			operator.key().as_ref(),
		],
	 	bump,
    )]
    pub user_roles_account: Account<'info, UserRoles>,

    // IncomingMessage PDA account
    // needs to be mutable as the validate_message CPI
    // updates its state
    #[account(
        init,
        seeds = [TRANSACTION_PDA_SEED],
        bump,
        payer = payer,
        space = {
            let mut bytes = Vec::with_capacity(256);
            relayer_transaction(None, None).serialize(&mut bytes)?;
            bytes.len()
        }
    )]
    pub transaction: AccountInfo<'info>,
}

pub fn initialize(
    ctx: Context<Initialize>,
    chain_name: String,
    its_hub_address: String,
) -> Result<()> {
    // Initialize ITS root
    *ctx.accounts.its_root_pda =
        InterchainTokenService::new(ctx.bumps.its_root_pda, chain_name, its_hub_address);

    // Initialize and assign OPERATOR role to the operator account.
    ctx.accounts.user_roles_account.roles = Roles::OPERATOR;
    ctx.accounts.user_roles_account.bump = ctx.bumps.user_roles_account;

    relayer_transaction(None, None)
        .serialize(&mut &mut ctx.accounts.transaction.data.borrow_mut()[..])?;

    Ok(())
}

/// Creates an Initialize instruction
pub fn make_initialize_instruction(
    payer: Pubkey,
    operator: Pubkey,
    chain_name: String,
    its_hub_address: String,
) -> (Instruction, crate::accounts::Initialize) {
    let its_root_pda = InterchainTokenService::find_pda().0;

    let program_data = bpf_loader_upgradeable::get_program_data_address(&crate::ID);

    let user_roles_account = UserRoles::find_pda(&its_root_pda, &operator).0;

    let transaction = relayer_discovery::find_transaction_pda(&crate::ID).0;

    let accounts = crate::accounts::Initialize {
        payer,
        program_data,
        its_root_pda,
        system_program: anchor_lang::system_program::ID,
        operator,
        user_roles_account,
        transaction,
    };

    (
        Instruction {
            program_id: crate::ID,
            accounts: accounts.to_account_metas(None),
            data: crate::instruction::Initialize {
                chain_name,
                its_hub_address,
            }
            .data(),
        },
        accounts,
    )
}
