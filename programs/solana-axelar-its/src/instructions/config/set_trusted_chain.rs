use crate::{
    events::TrustedChainSet,
    state::{InterchainTokenService, roles, RolesError, UserRoles},
    ItsError,
};
#[allow(deprecated)]
use anchor_lang::solana_program::bpf_loader_upgradeable;
use anchor_lang::solana_program::instruction::Instruction;
use anchor_lang::{prelude::*, InstructionData};

#[event_cpi]
#[derive(Accounts)]
#[instruction(chain_name: String)]
pub struct SetTrustedChain<'info> {
    /// Payer must be either the program upgrade authority or have the OPERATOR role.
    #[account(mut,
    	constraint =
     		user_roles.as_ref().is_some() || program_data.as_ref().is_some()
      	 		@ ItsError::MissingRequiredSignature,
    )]
    pub payer: Signer<'info>,

    /// The address of the account that will store the roles of the operator account.
    #[account(
	 	seeds = [
			UserRoles::SEED_PREFIX,
			its_root_pda.key().as_ref(),
			payer.key().as_ref(),
		],
	 	bump = user_roles.bump,
		// Require the payer to have the OPERATOR role.
		constraint = user_roles.contains(roles::OPERATOR) @ RolesError::MissingOperatorRole,
    )]
    pub user_roles: Option<Account<'info, UserRoles>>,

    #[account(
        seeds = [crate::ID.as_ref()],
        bump,
        seeds::program = bpf_loader_upgradeable::ID,
        constraint = program_data.upgrade_authority_address == Some(payer.key())
            @ ItsError::InvalidAccountData,
    )]
    pub program_data: Option<Account<'info, ProgramData>>,

    #[account(
    	mut,
    	realloc = its_root_pda.space_with_chain_added(&chain_name),
     	realloc::payer = payer,
      	realloc::zero = false,
     	seeds = [InterchainTokenService::SEED_PREFIX],
     	bump = its_root_pda.bump,
      	// Ensure the chain is not already added.
      	constraint = !its_root_pda.is_trusted_chain(&chain_name) @ ItsError::TrustedChainAlreadySet,
    )]
    pub its_root_pda: Account<'info, InterchainTokenService>,

    pub system_program: Program<'info, System>,
}

/// Sets a new trusted chain in the ITS configuration.
/// To authorize this action, the payer must be either the program upgrade authority
/// or have the OPERATOR role.
///
/// If both accounts are passed, the payer must be the program upgrade authority *and*
/// have the OPERATOR role.
pub fn set_trusted_chain(ctx: Context<SetTrustedChain>, chain_name: String) -> Result<()> {
    ctx.accounts
        .its_root_pda
        .add_trusted_chain(chain_name.clone());

    emit_cpi!(TrustedChainSet { chain_name });

    Ok(())
}

/// Creates a SetTrustedChain instruction
pub fn make_set_trusted_chain_instruction(
    payer: Pubkey,
    chain_name: String,
    use_operator_role: bool,
) -> (Instruction, crate::accounts::SetTrustedChain) {
    let its_root_pda = InterchainTokenService::find_pda().0;

    let user_roles = use_operator_role.then(|| UserRoles::find_pda(&its_root_pda, &payer).0);

    let program_data = if use_operator_role {
        None
    } else {
        Some(bpf_loader_upgradeable::get_program_data_address(&crate::ID))
    };

    let (event_authority, _) = Pubkey::find_program_address(&[b"__event_authority"], &crate::ID);

    let accounts = crate::accounts::SetTrustedChain {
        payer,
        user_roles,
        program_data,
        its_root_pda,
        system_program: anchor_lang::system_program::ID,
        event_authority,
        program: crate::ID,
    };

    (
        Instruction {
            program_id: crate::ID,
            accounts: accounts.to_account_metas(None),
            data: crate::instruction::SetTrustedChain { chain_name }.data(),
        },
        accounts,
    )
}
