use crate::gmp::*;
use crate::{
    errors::ItsError,
    events::InterchainTokenDeploymentStarted,
    seed_prefixes::INTERCHAIN_TOKEN_SEED,
    state::{InterchainTokenService, TokenManager},
    utils::{interchain_token_deployer_salt, interchain_token_id, interchain_token_id_internal},
};
use alloy_primitives::Bytes;
use anchor_lang::prelude::*;
use anchor_spl::token_2022::spl_token_2022::{
    extension::{metadata_pointer::MetadataPointer, BaseStateWithExtensions, StateWithExtensions},
    state::Mint as SplMint,
};
use anchor_spl::token_interface::Mint;
use interchain_token_transfer_gmp::{DeployInterchainToken, GMPPayload};
use mpl_token_metadata::accounts::Metadata;
use solana_axelar_gateway::program::SolanaAxelarGateway;
use solana_axelar_gateway::GatewayConfig;
use spl_token_metadata_interface::state::TokenMetadata;

/// Accounts required for deploying a remote interchain token
#[derive(Accounts)]
#[event_cpi]
#[instruction(salt: [u8; 32], destination_chain: String, gas_value: u64)]
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
        bump = token_manager_pda.bump,
        constraint = token_manager_pda.token_address == token_mint.key()  @ ItsError::TokenMintTokenManagerMissmatch
    )]
    pub token_manager_pda: Account<'info, TokenManager>,

    // GMP Accounts
    #[account(
        seeds = [
            solana_axelar_gateway::seed_prefixes::GATEWAY_SEED
        ],
        seeds::program = solana_axelar_gateway::ID,
        bump = gateway_root_pda.load()?.bump,
    )]
    pub gateway_root_pda: AccountLoader<'info, GatewayConfig>,

    /// The GMP gateway program account
    pub gateway_program: Program<'info, SolanaAxelarGateway>,

    pub system_program: Program<'info, System>,

    #[account(
        seeds = [InterchainTokenService::SEED_PREFIX],
        bump = its_root_pda.bump,
        constraint = !its_root_pda.paused @ ItsError::Paused,
        constraint = its_root_pda.chain_name != destination_chain @ ItsError::InvalidDestinationChain,
        constraint = its_root_pda.is_trusted_chain(&destination_chain) @ ItsError::UntrustedDestinationChain,
    )]
    pub its_root_pda: Account<'info, InterchainTokenService>,

    /// CHECK: validated in gateway
    pub call_contract_signing_pda: UncheckedAccount<'info>,

    // Event authority accounts
    #[account(
        seeds = [b"__event_authority"],
        bump,
        seeds::program = solana_axelar_gateway::ID,
    )]
    pub gateway_event_authority: AccountInfo<'info>,

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
            its_hub_address: self.its_root_pda.its_hub_address.clone(),
            call_contract_signing_pda: self.call_contract_signing_pda.to_account_info(),
            its_program: self.program.to_account_info(),
            gateway_event_authority: self.gateway_event_authority.to_account_info(),
            gas_event_authority: self
                .gas_service_accounts
                .gas_event_authority
                .to_account_info(),
        }
    }
}

pub fn deploy_remote_interchain_token_handler(
    ctx: Context<DeployRemoteInterchainToken>,
    salt: [u8; 32],
    destination_chain: String,
    gas_value: u64,
) -> Result<()> {
    let deploy_salt = interchain_token_deployer_salt(ctx.accounts.deployer.key, &salt);
    let token_id = interchain_token_id_internal(&deploy_salt);

    msg!("Instruction: OutboundDeploy");

    // get token metadata
    let (name, symbol) = get_token_metadata(
        &ctx.accounts.token_mint.to_account_info(),
        Some(&ctx.accounts.metadata_account),
    )?;

    let decimals = ctx.accounts.token_mint.decimals;

    emit_cpi!(InterchainTokenDeploymentStarted {
        token_id,
        token_name: name.clone(),
        token_symbol: symbol.clone(),
        token_decimals: decimals,
        minter: vec![],
        destination_chain: destination_chain.clone(),
    });

    let inner_payload = GMPPayload::DeployInterchainToken(DeployInterchainToken {
        selector: DeployInterchainToken::MESSAGE_TYPE_ID
            .try_into()
            .map_err(|_err| ItsError::ArithmeticOverflow)?,
        token_id: token_id.into(),
        name,
        symbol,
        decimals,
        minter: Bytes::default(),
    });

    let gmp_accounts = ctx.accounts.to_gmp_accounts();
    process_outbound(gmp_accounts, destination_chain, gas_value, inner_payload)?;

    Ok(())
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

    let metadata_account = maybe_metadata_account.ok_or(ItsError::NotEnoughAccountKeys)?;
    if *metadata_account.owner != mpl_token_metadata::ID {
        msg!("Invalid Metaplex metadata account");
        return err!(ItsError::InvalidMetaplexDataAccount);
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
