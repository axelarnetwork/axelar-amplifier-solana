use crate::gmp::*;
use crate::program::AxelarSolanaItsV2;
use crate::state::deploy_approval::DeployApproval;
use crate::state::UserRoles;
use crate::{
    errors::ItsError,
    events::InterchainTokenDeploymentStarted,
    seed_prefixes::INTERCHAIN_TOKEN_SEED,
    state::{InterchainTokenService, TokenManager},
    utils::{interchain_token_deployer_salt, interchain_token_id, interchain_token_id_internal},
};
use alloy_primitives::Bytes;
use anchor_lang::{prelude::*, solana_program};
use anchor_spl::token_2022::spl_token_2022::{
    extension::{metadata_pointer::MetadataPointer, BaseStateWithExtensions, StateWithExtensions},
    state::Mint as SplMint,
};
use anchor_spl::token_interface::Mint;
use axelar_solana_gas_service_v2::cpi::{accounts::PayGas, pay_gas};
use axelar_solana_gateway_v2::{seed_prefixes::CALL_CONTRACT_SIGNING_SEED, GatewayConfig};
use interchain_token_transfer_gmp::{DeployInterchainToken, GMPPayload, SendToHub};
use mpl_token_metadata::accounts::Metadata;
use spl_token_metadata_interface::state::TokenMetadata;

/// Accounts required for deploying a remote interchain token
#[derive(Accounts)]
#[event_cpi]
#[instruction(salt: [u8; 32], destination_chain: String, gas_value: u64, signing_pda_bump: u8)]
pub struct DeployRemoteInterchainToken<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    pub deployer: Signer<'info>,

    #[account(
        seeds = [
            INTERCHAIN_TOKEN_SEED,
            its_root_pda.key().as_ref(),
            &interchain_token_id(&deployer.key(), &salt)
        ],
        bump,
    )]
    pub token_mint: InterfaceAccount<'info, Mint>,

    /// CHECK: Decoded using get_token_metadata
    #[account(
        seeds = [
            b"metadata",
            mpl_token_metadata::ID.as_ref(),
            token_mint.key().as_ref()
        ],
        seeds::program = mpl_token_metadata::ID,
        bump
    )]
    pub metadata_account: AccountInfo<'info>,

    #[account(
        seeds = [
            crate::seed_prefixes::TOKEN_MANAGER_SEED,
            its_root_pda.key().as_ref(),
            &interchain_token_id(&deployer.key(), &salt)
        ],
        seeds::program = crate::ID,
        bump = token_manager_pda.bump,
    )]
    pub token_manager_pda: Account<'info, TokenManager>,

    // Optional Minter accounts
    pub minter: Option<Signer<'info>>,

    #[account(
        seeds = [
            DeployApproval::SEED_PREFIX,
            minter.as_ref().ok_or(ItsError::MinterNotProvided)?.key().as_ref(),
            &interchain_token_id(&deployer.key(), &salt),
            &anchor_lang::solana_program::keccak::hashv(&[destination_chain.as_bytes()]).to_bytes()
        ],
        bump = deploy_approval_pda.bump,
    )]
    pub deploy_approval_pda: Option<Account<'info, DeployApproval>>,

    #[account(
        seeds = [
            UserRoles::SEED_PREFIX,
            token_manager_pda.key().as_ref(),
            minter.as_ref().ok_or(ItsError::MinterNotProvided)?.key().as_ref()
        ],
        bump = minter_roles.bump,
        constraint = minter_roles.has_minter_role() @ ItsError::InvalidRole
    )]
    pub minter_roles: Option<Account<'info, UserRoles>>,

    // GMP Accounts
    #[account(
        seeds = [
            axelar_solana_gateway_v2::seed_prefixes::GATEWAY_SEED
        ],
        seeds::program = axelar_solana_gateway_v2::ID,
        bump = gateway_root_pda.load()?.bump,
    )]
    pub gateway_root_pda: AccountLoader<'info, GatewayConfig>,

    pub gateway_program: Program<'info, axelar_solana_gateway_v2::program::AxelarSolanaGatewayV2>,

    pub system_program: Program<'info, System>,

    #[account(
        seeds = [InterchainTokenService::SEED_PREFIX],
        bump = its_root_pda.bump,
        constraint = !its_root_pda.paused @ ItsError::Paused,
        constraint = its_root_pda.chain_name != destination_chain @ ItsError::InvalidDestinationChain,
        constraint = its_root_pda.is_trusted_chain_or_hub(&destination_chain) @ ItsError::UntrustedDestinationChain,
    )]
    pub its_root_pda: Account<'info, InterchainTokenService>,

    #[account(
        seeds = [CALL_CONTRACT_SIGNING_SEED],
        bump = signing_pda_bump,
        seeds::program = crate::ID
    )]
    pub call_contract_signing_pda: Signer<'info>,

    pub its_program: Program<'info, AxelarSolanaItsV2>,

    // Event authority accounts
    #[account(
        seeds = [b"__event_authority"],
        bump,
        seeds::program = axelar_solana_gateway_v2::ID,
    )]
    pub gateway_event_authority: SystemAccount<'info>,

    pub gas_service_accounts: GasServiceAccounts<'info>,
}

impl<'info> ToGMPAccounts<'info> for DeployRemoteInterchainToken<'info> {
    fn to_gmp_accounts(&self) -> GMPAccounts<'info> {
        GMPAccounts {
            payer: self.payer.to_account_info(),
            gateway_root_pda: self.gateway_root_pda.to_account_info(),
            gateway_program: self.gateway_program.to_account_info(),
            gas_treasury: self.gas_service_accounts.gas_treasury.to_account_info(),
            gas_service: self.gas_service_accounts.gas_service.to_account_info(),
            system_program: self.system_program.to_account_info(),
            its_root_pda: self.its_root_pda.clone(),
            call_contract_signing_pda: self.call_contract_signing_pda.to_account_info(),
            its_program: self.its_program.to_account_info(),
            gateway_event_authority: self.gateway_event_authority.to_account_info(),
            gas_event_authority: self.gas_service_accounts.gas_event_authority.to_account_info(),
        }
    }
}

pub fn deploy_remote_interchain_token_handler(
    ctx: Context<DeployRemoteInterchainToken>,
    salt: [u8; 32],
    destination_chain: String,
    gas_value: u64,
    signing_pda_bump: u8,
    maybe_destination_minter: Option<Vec<u8>>,
) -> Result<()> {
    let deploy_salt = interchain_token_deployer_salt(ctx.accounts.deployer.key, &salt);
    let token_id = interchain_token_id_internal(&deploy_salt);

    msg!("Instruction: OutboundDeploy");

    let destination_minter_data = if let Some(destination_minter) = maybe_destination_minter {
        let deploy_approval = ctx
            .accounts
            .deploy_approval_pda
            .as_ref()
            .ok_or(ItsError::DeployApprovalPDANotProvided)?;
        let minter = ctx
            .accounts
            .minter
            .as_ref()
            .ok_or(ItsError::MinterNotProvided)?
            .to_account_info();

        Some((Bytes::from(destination_minter), deploy_approval, minter))
    } else {
        None
    };

    // get token metadata
    let (name, symbol) = get_token_metadata(
        &ctx.accounts.token_mint.to_account_info(),
        Some(&ctx.accounts.metadata_account),
    )?;
    let decimals = ctx.accounts.token_mint.decimals;

    if ctx.accounts.token_manager_pda.token_address != ctx.accounts.token_mint.key() {
        msg!("TokenManager doesn't match mint");
        return err!(ItsError::InvalidArgument);
    }

    emit_cpi!(InterchainTokenDeploymentStarted {
        token_id,
        token_name: name.clone(),
        token_symbol: symbol.clone(),
        token_decimals: decimals,
        minter: destination_minter_data
            .as_ref()
            .map(|data| data.0.to_vec())
            .unwrap_or_default(),
        destination_chain: destination_chain.clone(),
    });

    let inner_payload = GMPPayload::DeployInterchainToken(DeployInterchainToken {
        selector: DeployInterchainToken::MESSAGE_TYPE_ID
            .try_into()
            .map_err(|_err| ProgramError::ArithmeticOverflow)?,
        token_id: token_id.into(),
        name,
        symbol,
        decimals,
        minter: destination_minter_data
            .as_ref()
            .map(|data| data.0.clone())
            .unwrap_or_default(),
    });

    let gmp_accounts = ctx.accounts.to_gmp_accounts();
    process_outbound(
        gmp_accounts,
        destination_chain,
        gas_value,
        signing_pda_bump,
        inner_payload,
    )?;

    Ok(())
}

pub fn process_outbound(
    gmp_accounts: GMPAccounts,
    destination_chain: String,
    gas_value: u64,
    signing_pda_bump: u8,
    inner_payload: GMPPayload,
) -> Result<()> {
    // Wrap the inner payload
    let payload = GMPPayload::SendToHub(SendToHub {
        selector: SendToHub::MESSAGE_TYPE_ID
            .try_into()
            .map_err(|_err| ProgramError::ArithmeticOverflow)?,
        destination_chain: destination_chain.clone(),
        payload: inner_payload.encode().into(),
    })
    .encode();

    let payload_hash = solana_program::keccak::hash(&payload).to_bytes();

    if gas_value > 0 {
        pay_gas_v2(
            gmp_accounts.clone(),
            payload_hash,
            destination_chain.clone(),
            gmp_accounts.its_root_pda.its_hub_address.clone(),
            gas_value,
        )?;
    }

    // Call contract instruction

    let destination_address = gmp_accounts.its_root_pda.its_hub_address.clone();

    let signer_seeds: &[&[&[u8]]] = &[&[CALL_CONTRACT_SIGNING_SEED, &[signing_pda_bump]]];

    let cpi_accounts = axelar_solana_gateway_v2::cpi::accounts::CallContract {
        caller: gmp_accounts.its_program.to_account_info(),
        signing_pda: Some(gmp_accounts.call_contract_signing_pda.to_account_info()),
        gateway_root_pda: gmp_accounts.gateway_root_pda.to_account_info(),
        // For event_cpi
        event_authority: gmp_accounts.gateway_event_authority.to_account_info(),
        program: gmp_accounts.gateway_program.to_account_info(),
    };

    let cpi_ctx = CpiContext::new_with_signer(
        gmp_accounts.gateway_program.to_account_info(),
        cpi_accounts,
        signer_seeds,
    );

    axelar_solana_gateway_v2::cpi::call_contract(
        cpi_ctx,
        destination_chain,
        destination_address,
        payload,
        signing_pda_bump,
    )?;

    Ok(())
}

fn pay_gas_v2(
    gmp_accounts: GMPAccounts,
    payload_hash: [u8; 32],
    destination_chain: String,
    destination_address: String,
    gas_value: u64,
) -> Result<()> {
    let cpi_accounts = PayGas {
        sender: gmp_accounts.payer.clone(),
        treasury: gmp_accounts.gas_treasury,
        system_program: gmp_accounts.system_program,
        event_authority: gmp_accounts.gas_event_authority,
        program: gmp_accounts.gas_service.clone(),
    };

    let cpi_ctx = CpiContext::new(gmp_accounts.gas_service, cpi_accounts);

    pay_gas(
        cpi_ctx,
        destination_chain,
        destination_address,
        payload_hash,
        gas_value,
        gmp_accounts.payer.key(), // refund_address
    )
}

/// Retrieves token metadata with fallback logic:
/// 1. First, try to get metadata from Token 2022 extensions
/// 2. If we can't retrieve the metadata from embedded TokenMetadata, we try to deserialize the
///    data from the given metadata account, if any, as Metaplex `Metadata`.
pub(crate) fn get_token_metadata(
    mint: &AccountInfo,
    maybe_metadata_account: Option<&AccountInfo>,
) -> Result<(String, String)> {
    let mint_data = mint.try_borrow_data()?;

    if let Ok(mint_with_extensions) = StateWithExtensions::<SplMint>::unpack(&mint_data) {
        if let Ok(metadata_pointer) = mint_with_extensions.get_extension::<MetadataPointer>() {
            if let Some(metadata_address) =
                Option::<Pubkey>::from(metadata_pointer.metadata_address)
            {
                if metadata_address == *mint.key {
                    if let Ok(token_metadata_ext) =
                        mint_with_extensions.get_variable_len_extension::<TokenMetadata>()
                    {
                        return Ok((token_metadata_ext.name, token_metadata_ext.symbol));
                    }
                }
            }
        }
    }

    let metadata_account = maybe_metadata_account.ok_or(ProgramError::NotEnoughAccountKeys)?;
    if *metadata_account.owner != mpl_token_metadata::ID {
        msg!("Invalid Metaplex metadata account");
        return err!(ItsError::InvalidAccountOwner);
    }

    let token_metadata = Metadata::from_bytes(&metadata_account.try_borrow_data()?)?;
    if token_metadata.mint != *mint.key {
        msg!("The metadata and mint accounts passed don't match");
        return err!(ItsError::InvalidArgument);
    }

    let name = token_metadata.name.trim_end_matches('\0').to_owned();
    let symbol = token_metadata.symbol.trim_end_matches('\0').to_owned();

    Ok((name, symbol))
}
