use crate::{errors::ItsError, instructions::gmp::*, state::InterchainTokenService};
use anchor_lang::{prelude::*, solana_program, Key};
use interchain_token_transfer_gmp::GMPPayload;
use interchain_token_transfer_gmp::ReceiveFromHub;
use interchain_token_transfer_gmp::SendToHub;
use solana_axelar_gateway::{executable::validate_message_raw, Message};
use solana_program::instruction::AccountMeta;
use solana_program::instruction::Instruction;

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
    ctx: Context<'_, '_, '_, 'info, Execute<'info>>,
    message: Message,
    payload: Vec<u8>,
) -> Result<()> {
    use GMPPayload::{
        DeployInterchainToken, InterchainTransfer, LinkToken, ReceiveFromHub,
        RegisterTokenMetadata, SendToHub,
    };

    // Verify that the message comes from the trusted Axelar ITS Hub
    if message.source_address != ctx.accounts.its_root_pda.its_hub_address {
        msg!("Untrusted source address: {}", message.source_address);
        return err!(ItsError::InvalidInstructionData);
    }

    // Execute can only be called with ReceiveFromHub payload at the top level
    let ReceiveFromHub(inner_msg) =
        GMPPayload::decode(&payload).map_err(|_err| ItsError::InvalidInstructionData)?
    else {
        msg!("Unsupported GMP payload");
        return err!(ItsError::InvalidInstructionData);
    };
    // Validate the GMP message
    validate_message_raw(
        &(&ctx.accounts.executable).into(),
        message.clone(),
        &payload,
    )?;

    if !ctx
        .accounts
        .its_root_pda
        .is_trusted_chain(&inner_msg.source_chain)
    {
        return err!(ItsError::UntrustedSourceChain);
    }

    let payload =
        GMPPayload::decode(&inner_msg.payload).map_err(|_err| ItsError::InvalidInstructionData)?;

    let source_chain = &inner_msg.source_chain;

    match payload {
        InterchainTransfer(transfer) => {
            cpi_execute_interchain_transfer(ctx, transfer, message, source_chain)
        }
        DeployInterchainToken(deploy) => cpi_execute_deploy_interchain_token(ctx, deploy),
        LinkToken(payload) => cpi_execute_link_token(ctx, payload),
        SendToHub(_) | ReceiveFromHub(_) | RegisterTokenMetadata(_) => {
            err!(ItsError::InvalidInstructionData)
        }
    }
}

fn cpi_execute_interchain_transfer<'info>(
    ctx: Context<'_, '_, '_, 'info, Execute<'info>>,
    transfer: interchain_token_transfer_gmp::InterchainTransfer,
    message: Message,
    source_chain: &str,
) -> Result<()> {
    let token_id = transfer.token_id.0;
    let source_address = String::from_utf8(transfer.source_address.to_vec())
        .map_err(|_| ItsError::InvalidInstructionData)?;

    let destination_address: [u8; 32] = transfer
        .destination_address
        .as_ref()
        .try_into()
        .map_err(|_| ItsError::InvalidAccountData)?;
    let destination_address = Pubkey::new_from_array(destination_address);

    let amount: u64 = transfer
        .amount
        .try_into()
        .map_err(|_| ItsError::ArithmeticOverflow)?;

    let data = transfer.data;

    let mut remaining = ctx.remaining_accounts.iter();
    let destination = remaining.next().ok_or(ItsError::AccountNotProvided)?;
    let destination_ata = remaining.next().ok_or(ItsError::AccountNotProvided)?;

    Ok(())
}

fn cpi_execute_link_token<'info>(
    ctx: Context<'_, '_, '_, 'info, Execute<'info>>,
    payload: interchain_token_transfer_gmp::LinkToken,
) -> Result<()> {
    let token_id = payload.token_id.0;

    let destination_token_address: [u8; 32] = payload
        .destination_token_address
        .as_ref()
        .try_into()
        .map_err(|_| ItsError::InvalidAccountData)?;

    let token_manager_type: u8 = payload
        .token_manager_type
        .try_into()
        .map_err(|_| ItsError::ArithmeticOverflow)?; // U256 to u8

    let link_params = payload.link_params.to_vec(); // Vec<u8>

    let mut remaining = ctx.remaining_accounts.iter();
    let deployer = remaining.next().ok_or(ItsError::AccountNotProvided)?;
    let minter = remaining.next();
    let minter_roles_pda = remaining.next();

    Ok(())
}

fn cpi_execute_deploy_interchain_token<'info>(
    ctx: Context<'_, '_, '_, 'info, Execute<'info>>,
    deploy: interchain_token_transfer_gmp::DeployInterchainToken,
) -> Result<()> {
    // Extract data from the deploy payload
    let token_id = deploy.token_id.0;
    let name = deploy.name;
    let symbol = deploy.symbol;
    let decimals = deploy.decimals;
    let minter = deploy.minter;

    let mut remaining = ctx.remaining_accounts.iter();
    let deployer_ata = remaining.next().ok_or(ItsError::AccountNotProvided)?;
    let deployer = remaining.next().ok_or(ItsError::AccountNotProvided)?;
    let sysvar_instructions = remaining.next().ok_or(ItsError::AccountNotProvided)?;
    let mpl_token_metadata_program = remaining.next().ok_or(ItsError::AccountNotProvided)?;
    let mpl_token_metadata_account = remaining.next().ok_or(ItsError::AccountNotProvided)?;
    let minter = remaining.next();
    let minter_roles_pda = remaining.next();

    Ok(())
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
) -> Vec<AccountMeta> {
    vec![
        AccountMeta::new(destination, false),
        AccountMeta::new(destination_ata, false),
    ]
}

/// Helper function to build the extra accounts needed for execute with LinkToken payload.
///
/// Usage:
/// ```ignore
/// let mut accounts = solana_axelar_its::accounts::Execute { ... }.to_account_metas(None);
/// accounts.extend(execute_link_token_extra_accounts(deployer, minter, minter_roles_pda));
/// ```
pub fn execute_link_token_extra_accounts(
    deployer: Pubkey,
    operator: Option<Pubkey>,
    operator_roles_pda: Option<Pubkey>,
) -> Vec<AccountMeta> {
    let mut accounts = vec![AccountMeta::new(deployer, false)];

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
///     deployer_ata,
///     deployer,
///     sysvar_instructions,
///     mpl_token_metadata_program,
///     mpl_token_metadata_account,
///     minter,
///     minter_roles_pda,
/// ));
/// ```
pub fn execute_deploy_interchain_token_extra_accounts(
    deployer_ata: Pubkey,
    deployer: Pubkey,
    sysvar_instructions: Pubkey,
    mpl_token_metadata_program: Pubkey,
    mpl_token_metadata_account: Pubkey,
    minter: Option<Pubkey>,
    minter_roles_pda: Option<Pubkey>,
) -> Vec<AccountMeta> {
    let mut accounts = vec![
        AccountMeta::new(deployer_ata, false),
        AccountMeta::new(deployer, false),
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
