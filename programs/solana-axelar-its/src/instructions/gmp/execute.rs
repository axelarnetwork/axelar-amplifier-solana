use crate::encoding::{HubMessage, Message as ItsMessage};
use crate::{errors::ItsError, state::InterchainTokenService, InterchainTransferExecute};
use anchor_lang::{prelude::*, solana_program, InstructionData, Key};
use solana_axelar_gateway::{
    executable::{validate_message_raw, HasAxelarExecutable},
    executable_accounts, Message as CrossChainMessage,
};
use solana_program::instruction::AccountMeta;
use solana_program::instruction::Instruction;

executable_accounts!(Execute);

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

    /// CHECK:
    #[account(mut)]
    pub token_manager_pda: UncheckedAccount<'info>,

    /// CHECK:
    #[account(mut)]
    pub token_mint: UncheckedAccount<'info>,

    /// CHECK:
    #[account(mut)]
    pub token_manager_ata: UncheckedAccount<'info>,

    /// CHECK:
    pub token_program: UncheckedAccount<'info>,

    /// CHECK:
    pub associated_token_program: UncheckedAccount<'info>,

    /// CHECK:
    pub system_program: UncheckedAccount<'info>,
}

pub fn execute_handler<'info>(
    ctx: Context<'_, '_, '_, 'info, Execute<'info>>,
    message: CrossChainMessage,
    payload: Vec<u8>,
) -> Result<()> {
    use ItsMessage::{DeployInterchainToken, InterchainTransfer, LinkToken};

    // Verify that the message comes from the trusted Axelar ITS Hub
    if message.source_address != ctx.accounts.its_root_pda.its_hub_address {
        msg!("Untrusted source address: {}", message.source_address);
        return err!(ItsError::InvalidInstructionData);
    }

    let hub_message =
        HubMessage::try_from_slice(&payload).map_err(|_err| ItsError::InvalidInstructionData)?;

    // Execute can only be called with ReceiveFromHub payload at the top level
    let HubMessage::ReceiveFromHub {
        source_chain,
        message: inner_message,
    } = hub_message
    else {
        msg!("Unsupported GMP payload");
        return err!(ItsError::InvalidInstructionData);
    };

    // Validate the GMP message
    validate_message_raw(&ctx.accounts.axelar_executable(), message.clone(), &payload)?;

    if !ctx.accounts.its_root_pda.is_trusted_chain(&source_chain) {
        return err!(ItsError::UntrustedSourceChain);
    }

    match inner_message {
        InterchainTransfer(transfer) => {
            cpi_execute_interchain_transfer(ctx, transfer, message, &source_chain)
        }
        DeployInterchainToken(deploy) => cpi_execute_deploy_interchain_token(ctx, deploy),
        LinkToken(payload) => cpi_execute_link_token(ctx, payload),
    }
}

fn cpi_execute_interchain_transfer<'info>(
    ctx: Context<'_, '_, '_, 'info, Execute<'info>>,
    transfer: crate::encoding::InterchainTransfer,
    message: CrossChainMessage,
    source_chain: &str,
) -> Result<()> {
    let token_id = transfer.token_id;

    let destination_address: [u8; 32] = transfer
        .destination_address
        .as_slice()
        .try_into()
        .map_err(|_| ItsError::InvalidAccountData)?;
    let destination_address = Pubkey::new_from_array(destination_address);

    let data = transfer.data.unwrap_or_default();

    let instruction_data = crate::instruction::ExecuteInterchainTransfer {
        token_id,
        source_address: transfer.source_address,
        destination_address,
        amount: transfer.amount,
        data,
        message,
        source_chain: source_chain.to_owned(),
    };

    let mut remaining = ctx.remaining_accounts.iter();
    let destination = remaining.next().ok_or(ItsError::AccountNotProvided)?;
    let destination_ata = remaining.next().ok_or(ItsError::AccountNotProvided)?;
    // Optional interchain transfer execute
    let interchain_transfer_execute = remaining.next();

    let custom_accounts: Vec<_> = remaining.cloned().collect();

    let mut accounts = crate::accounts::ExecuteInterchainTransfer {
        payer: ctx.accounts.payer.key(),
        its_root_pda: ctx.accounts.its_root_pda.key(),
        destination: destination.key(),
        destination_ata: destination_ata.key(),
        token_mint: ctx.accounts.token_mint.key(),
        token_manager_pda: ctx.accounts.token_manager_pda.key(),
        token_manager_ata: ctx.accounts.token_manager_ata.key(),
        token_program: ctx.accounts.token_program.key(),
        associated_token_program: ctx.accounts.associated_token_program.key(),
        system_program: ctx.accounts.system_program.key(),
        event_authority: ctx.accounts.event_authority.key(),
        program: ctx.accounts.program.key(),
        interchain_transfer_execute: interchain_transfer_execute.map(Key::key),
    }
    .to_account_metas(None);
    // Optional destination program custom accounts
    accounts.extend(
        custom_accounts
            .iter()
            .flat_map(|a| a.to_account_metas(None)),
    );

    let transfer_instruction = Instruction {
        program_id: crate::id(),
        accounts,
        data: instruction_data.data(),
    };

    let mut account_infos =
        crate::__cpi_client_accounts_execute_interchain_transfer::ExecuteInterchainTransfer {
            payer: ctx.accounts.payer.to_account_info(),
            its_root_pda: ctx.accounts.its_root_pda.to_account_info(),
            destination: destination.to_account_info(),
            destination_ata: destination_ata.to_account_info(),
            token_mint: ctx.accounts.token_mint.to_account_info(),
            token_manager_pda: ctx.accounts.token_manager_pda.to_account_info(),
            token_manager_ata: ctx.accounts.token_manager_ata.to_account_info(),
            token_program: ctx.accounts.token_program.to_account_info(),
            associated_token_program: ctx.accounts.associated_token_program.to_account_info(),
            system_program: ctx.accounts.system_program.to_account_info(),
            event_authority: ctx.accounts.event_authority.to_account_info(),
            program: ctx.accounts.program.to_account_info(),
            interchain_transfer_execute: interchain_transfer_execute.cloned(),
        }
        .to_account_infos();

    // Optional destination program custom accounts
    account_infos.extend(custom_accounts);

    // Invoke the instruction with ITS root PDA as signer
    invoke_signed_with_its_root_pda(
        &transfer_instruction,
        &account_infos,
        ctx.accounts.its_root_pda.bump,
    )
}

fn cpi_execute_link_token<'info>(
    ctx: Context<'_, '_, '_, 'info, Execute<'info>>,
    payload: crate::encoding::LinkToken,
) -> Result<()> {
    let token_id = payload.token_id;

    let destination_token_address: [u8; 32] = payload
        .destination_token_address
        .as_slice()
        .try_into()
        .map_err(|_| ItsError::InvalidAccountData)?;

    let token_manager_type = payload.token_manager_type;

    let link_params = payload.params.unwrap_or_default();

    // Create the instruction data using Anchor's InstructionData trait
    let instruction_data = crate::instruction::ExecuteLinkToken {
        token_id,
        destination_token_address,
        token_manager_type,
        link_params,
    };

    let mut remaining = ctx.remaining_accounts.iter();
    let operator = remaining.next();
    let operator_roles_pda = remaining.next();

    let accounts = crate::accounts::ExecuteLinkToken {
        payer: ctx.accounts.payer.key(),
        system_program: ctx.accounts.system_program.key(),
        its_root_pda: ctx.accounts.its_root_pda.key(),
        token_manager_pda: ctx.accounts.token_manager_pda.key(),
        token_mint: ctx.accounts.token_mint.key(),
        token_manager_ata: ctx.accounts.token_manager_ata.key(),
        token_program: ctx.accounts.token_program.key(),
        associated_token_program: ctx.accounts.associated_token_program.key(),
        operator: operator.map(Key::key),
        operator_roles_pda: operator_roles_pda.map(Key::key),
        // for event cpi
        event_authority: ctx.accounts.event_authority.key(),
        program: ctx.accounts.program.key(),
    };

    // Create the instruction
    let link_instruction = Instruction {
        program_id: crate::id(),
        accounts: accounts.to_account_metas(None),
        data: instruction_data.data(),
    };

    let account_infos = crate::__cpi_client_accounts_execute_link_token::ExecuteLinkToken {
        payer: ctx.accounts.payer.to_account_info(),
        system_program: ctx.accounts.system_program.to_account_info(),
        its_root_pda: ctx.accounts.its_root_pda.to_account_info(),
        token_manager_pda: ctx.accounts.token_manager_pda.to_account_info(),
        token_mint: ctx.accounts.token_mint.to_account_info(),
        token_manager_ata: ctx.accounts.token_manager_ata.to_account_info(),
        token_program: ctx.accounts.token_program.to_account_info(),
        associated_token_program: ctx.accounts.associated_token_program.to_account_info(),
        operator: operator.cloned(),
        operator_roles_pda: operator_roles_pda.cloned(),
        event_authority: ctx.accounts.event_authority.to_account_info(),
        program: ctx.accounts.program.to_account_info(),
    }
    .to_account_infos();

    invoke_signed_with_its_root_pda(
        &link_instruction,
        &account_infos,
        ctx.accounts.its_root_pda.bump,
    )
}

fn cpi_execute_deploy_interchain_token<'info>(
    ctx: Context<'_, '_, '_, 'info, Execute<'info>>,
    deploy: crate::encoding::DeployInterchainToken,
) -> Result<()> {
    // Extract data from the deploy payload
    let token_id = deploy.token_id;
    let name = deploy.name;
    let symbol = deploy.symbol;
    let decimals = deploy.decimals;
    let minter = deploy.minter.unwrap_or_default();

    // Create the instruction data using Anchor's InstructionData trait
    let instruction_data = crate::instruction::ExecuteDeployInterchainToken {
        token_id,
        name,
        symbol,
        decimals,
        minter,
    };

    let mut remaining = ctx.remaining_accounts.iter();
    let sysvar_instructions = remaining.next().ok_or(ItsError::AccountNotProvided)?;
    let mpl_token_metadata_program = remaining.next().ok_or(ItsError::AccountNotProvided)?;
    let mpl_token_metadata_account = remaining.next().ok_or(ItsError::AccountNotProvided)?;
    let minter = remaining.next();
    let minter_roles_pda = remaining.next();

    // Build the accounts using Anchor's generated accounts struct
    let accounts = crate::accounts::ExecuteDeployInterchainToken {
        payer: ctx.accounts.payer.key(),
        system_program: ctx.accounts.system_program.key(),
        its_root_pda: ctx.accounts.its_root_pda.key(),
        token_manager_pda: ctx.accounts.token_manager_pda.key(),
        token_mint: ctx.accounts.token_mint.key(),
        token_manager_ata: ctx.accounts.token_manager_ata.key(),
        token_program: ctx.accounts.token_program.key(),
        associated_token_program: ctx.accounts.associated_token_program.key(),
        sysvar_instructions: sysvar_instructions.key(),
        mpl_token_metadata_program: mpl_token_metadata_program.key(),
        mpl_token_metadata_account: mpl_token_metadata_account.key(),
        minter: minter.map(Key::key),
        minter_roles_pda: minter_roles_pda.map(Key::key),
        event_authority: ctx.accounts.event_authority.key(),
        program: ctx.accounts.program.key(),
    };

    // Create the instruction
    let deploy_instruction = Instruction {
        program_id: crate::id(),
        accounts: accounts.to_account_metas(None),
        data: instruction_data.data(),
    };

    let account_infos = crate::__cpi_client_accounts_execute_deploy_interchain_token::ExecuteDeployInterchainToken {
		payer: ctx.accounts.payer.to_account_info(),
		system_program: ctx.accounts.system_program.to_account_info(),
		its_root_pda: ctx.accounts.its_root_pda.to_account_info(),
		token_manager_pda: ctx.accounts.token_manager_pda.to_account_info(),
		token_mint: ctx.accounts.token_mint.to_account_info(),
		token_manager_ata: ctx.accounts.token_manager_ata.to_account_info(),
		token_program: ctx.accounts.token_program.to_account_info(),
		associated_token_program: ctx.accounts.associated_token_program.to_account_info(),
		sysvar_instructions: sysvar_instructions.to_account_info(),
		mpl_token_metadata_program: mpl_token_metadata_program.to_account_info(),
		mpl_token_metadata_account: mpl_token_metadata_account.to_account_info(),
		minter: minter
			.cloned(),
		minter_roles_pda: minter_roles_pda
			.cloned(),
		event_authority: ctx.accounts.event_authority.to_account_info(),
		program: ctx.accounts.program.to_account_info(),
	}.to_account_infos();

    invoke_signed_with_its_root_pda(
        &deploy_instruction,
        &account_infos,
        ctx.accounts.its_root_pda.bump,
    )
}

fn invoke_signed_with_its_root_pda(
    instruction: &Instruction,
    account_infos: &[AccountInfo],
    its_root_pda_bump: u8,
) -> Result<()> {
    let seeds = &[InterchainTokenService::SEED_PREFIX, &[its_root_pda_bump]];
    let signer_seeds = &[&seeds[..]];

    anchor_lang::solana_program::program::invoke_signed(instruction, account_infos, signer_seeds)?;
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
        AccountMeta::new_readonly(destination, false),
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
        accounts.push(AccountMeta::new_readonly(key, false));
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
