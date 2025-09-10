use crate::state::InterchainTokenService;
use anchor_lang::prelude::*;
#[allow(deprecated)]
use anchor_lang::solana_program::bpf_loader_upgradeable;
use axelar_solana_operators::OperatorAccount;

/// Initialize the configuration PDA.
#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        seeds = [crate::ID.as_ref()],
        bump,
        seeds::program = bpf_loader_upgradeable::ID,
        constraint = program_data.upgrade_authority_address == Some(payer.key()) @ ProgramError::InvalidAccountOwner
    )]
    pub program_data: Account<'info, ProgramData>,

    #[account(
    	init,
     	space = InterchainTokenService::DISCRIMINATOR.len() + InterchainTokenService::INIT_SPACE,
      	payer = payer,
     	seeds = [InterchainTokenService::SEED_PREFIX],
     	bump,
    )]
    pub its_root_pda_account: Account<'info, InterchainTokenService>,

    pub system_program: Program<'info, System>,

    pub operator: Signer<'info>,

    #[account(
        seeds = [
            OperatorAccount::SEED_PREFIX,
            operator.key().as_ref(),
        ],
        bump = operator_pda.bump,
        seeds::program = axelar_solana_operators::ID
    )]
    pub operator_pda: Account<'info, OperatorAccount>,

    /// The address of the account that will store the roles of the operator account.
    #[account(mut)]
    // TODO(v2) make user roles account
    pub user_roles_account: UncheckedAccount<'info>,
}

pub fn initialize(
    ctx: Context<Initialize>,
    chain_name: String,
    its_hub_address: String,
) -> Result<()> {
    Ok(())
}
