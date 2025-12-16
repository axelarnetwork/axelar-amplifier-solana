use crate::{
    errors::ItsError,
    instructions::gmp::*,
    state::{InterchainTokenService, InterchainTransferExecute},
};
use anchor_lang::{prelude::*, solana_program, Key};
use interchain_token_transfer_gmp::GMPPayload;
use interchain_token_transfer_gmp::ReceiveFromHub;
use solana_axelar_gateway::{executable::validate_message_raw, Message};
use solana_program::instruction::AccountMeta;

pub fn validate_message<'info>(
    executable: &AxelarExecuteAccounts<'info>,
    its_root_pda: &Account<'info, InterchainTokenService>,
    message: Message,
    inner_payload: GMPPayload,
    source_chain: String,
) -> Result<()> {
    let payload = GMPPayload::ReceiveFromHub(ReceiveFromHub {
        selector: ReceiveFromHub::MESSAGE_TYPE_ID
            .try_into()
            .map_err(|_err| ItsError::ArithmeticOverflow)?,
        source_chain: source_chain.clone(),
        payload: inner_payload.encode().into(),
    })
    .encode();
    // Validate the GMP message
    validate_message_raw(&executable.into(), message.clone(), &payload)?;

    // Verify that the message comes from the trusted Axelar ITS Hub
    if message.source_address != its_root_pda.its_hub_address {
        msg!("Untrusted source address: {}", message.source_address);
        return err!(ItsError::InvalidInstructionData);
    }

    if !its_root_pda.is_trusted_chain(&source_chain) {
        return err!(ItsError::UntrustedSourceChain);
    }

    Ok(())
}

#[derive(Accounts)]
#[event_cpi]
pub struct Execute<'info> {
    // GMP Accounts
    pub executable: AxelarExecuteAccounts<'info>,

    // ITS Accounts
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        seeds = [InterchainTokenService::SEED_PREFIX],
        bump = its_root_pda.bump,
        constraint = !its_root_pda.paused @ ItsError::Paused,
    )]
    pub its_root_pda: Account<'info, InterchainTokenService>,

    #[account(mut)]
    pub token_manager_pda: UncheckedAccount<'info>,

    #[account(mut)]
    pub token_mint: UncheckedAccount<'info>,

    #[account(mut)]
    pub token_manager_ata: UncheckedAccount<'info>,

    pub token_program: UncheckedAccount<'info>,

    pub associated_token_program: UncheckedAccount<'info>,

    pub system_program: UncheckedAccount<'info>,
}

pub fn execute_handler<'info>(
    _ctx: Context<'_, '_, '_, 'info, Execute<'info>>,
    _message: Message,
    _payload: Vec<u8>,
) -> Result<()> {
    Ok(())
}

/// Helper function to build the extra accounts needed for execute with InterchainTransfer payload.
///
/// Usage:
/// ```ignore
/// let mut accounts = solana_axelar_its::accounts::Execute { ... }.to_account_metas(None);
/// accounts.extend(execute_interchain_transfer_extra_accounts(destination, destination_ata));
/// ```
pub fn execute_interchain_transfer_extra_accounts(
    destination: Pubkey,
    destination_ata: Pubkey,
    transfer_has_data: Option<bool>,
) -> Vec<AccountMeta> {
    let mut accounts = vec![
        AccountMeta::new(destination, false),
        AccountMeta::new(destination_ata, false),
    ];

    if transfer_has_data == Some(true) {
        let interchain_transfer_execute = Pubkey::find_program_address(
            &[InterchainTransferExecute::SEED_PREFIX, destination.as_ref()],
            &crate::ID,
        )
        .0;
        accounts.push(AccountMeta::new_readonly(
            interchain_transfer_execute,
            false,
        ));
    }

    accounts
}

/// Helper function to build the extra accounts needed for execute with LinkToken payload.
///
/// Usage:
/// ```ignore
/// let mut accounts = solana_axelar_its::accounts::Execute { ... }.to_account_metas(None);
/// accounts.extend(execute_link_token_extra_accounts(minter, minter_roles_pda));
/// ```
pub fn execute_link_token_extra_accounts(
    operator: Option<Pubkey>,
    operator_roles_pda: Option<Pubkey>,
) -> Vec<AccountMeta> {
    let mut accounts = Vec::with_capacity(2);

    if let Some(key) = operator {
        accounts.push(AccountMeta::new(key, false));
    }

    if let Some(pda_key) = operator_roles_pda {
        accounts.push(AccountMeta::new(pda_key, false));
    }

    accounts
}

/// Helper function to build the extra accounts needed for execute with DeployInterchainToken payload.
///
/// Usage:
/// ```ignore
/// let mut accounts = solana_axelar_its::accounts::Execute { ... }.to_account_metas(None);
/// accounts.extend(execute_deploy_interchain_token_extra_accounts(
///     sysvar_instructions,
///     mpl_token_metadata_program,
///     mpl_token_metadata_account,
///     minter,
///     minter_roles_pda,
/// ));
/// ```
pub fn execute_deploy_interchain_token_extra_accounts(
    sysvar_instructions: Pubkey,
    mpl_token_metadata_program: Pubkey,
    mpl_token_metadata_account: Pubkey,
    minter: Option<Pubkey>,
    minter_roles_pda: Option<Pubkey>,
) -> Vec<AccountMeta> {
    let mut accounts = vec![
        AccountMeta::new_readonly(sysvar_instructions, false),
        AccountMeta::new_readonly(mpl_token_metadata_program, false),
        AccountMeta::new(mpl_token_metadata_account, false),
    ];

    if let Some(minter_key) = minter {
        accounts.push(AccountMeta::new(minter_key, false));
    }

    if let Some(minter_roles_pda_key) = minter_roles_pda {
        accounts.push(AccountMeta::new(minter_roles_pda_key, false));
    }

    accounts
}
