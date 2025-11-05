use crate::gmp::*;
use crate::instructions::deploy_remote_interchain_token::get_token_metadata;
use crate::{
    errors::ItsError,
    events::InterchainTokenDeploymentStarted,
    state::{InterchainTokenService, TokenManager},
    utils::{
        canonical_interchain_token_deploy_salt, canonical_interchain_token_id,
        interchain_token_id_internal,
    },
};
use alloy_primitives::Bytes;
use anchor_lang::prelude::*;
use anchor_spl::token_interface::Mint;
use interchain_token_transfer_gmp::{DeployInterchainToken, GMPPayload};
use solana_axelar_gateway::GatewayConfig;

/// Accounts required for deploying a remote canonical interchain token
#[derive(Accounts)]
#[event_cpi]
#[instruction(destination_chain: String, gas_value: u64)]
pub struct DeployRemoteCanonicalInterchainToken<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    /// CHECK: we can't check this mint since we didn't deploy it
    /// The check happens in the token_manager, where we know that its a valid
    /// token manager deployed for this token_mint
    pub token_mint: InterfaceAccount<'info, Mint>,

    /// CHECK: decoded using get_token_metadata
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
            &canonical_interchain_token_id(&token_mint.key())
        ],
        bump = token_manager_pda.bump,
        constraint = token_manager_pda.token_address == token_mint.key()  @ ItsError::InvalidTokenManagerPda
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
    pub gateway_program: Program<'info, solana_axelar_gateway::program::SolanaAxelarGateway>,

    pub system_program: Program<'info, System>,

    #[account(
        seeds = [InterchainTokenService::SEED_PREFIX],
        bump = its_root_pda.bump,
        constraint = !its_root_pda.paused @ ItsError::Paused,
        constraint = its_root_pda.chain_name != destination_chain @ ItsError::InvalidDestinationChain,
        constraint = its_root_pda.is_trusted_chain_or_hub(&destination_chain) @ ItsError::UntrustedDestinationChain,
    )]
    pub its_root_pda: Account<'info, InterchainTokenService>,

    /// CHECK: validated in gateway
    pub call_contract_signing_pda: UncheckedAccount<'info>,

    #[account(
        seeds = [b"__event_authority"],
        bump,
        seeds::program = solana_axelar_gateway::ID,
    )]
    pub gateway_event_authority: SystemAccount<'info>,

    pub gas_service_accounts: GasServiceAccounts<'info>,
}

impl<'info> ToGMPAccounts<'info> for DeployRemoteCanonicalInterchainToken<'info> {
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

pub fn deploy_remote_canonical_interchain_token_handler(
    ctx: Context<DeployRemoteCanonicalInterchainToken>,
    destination_chain: String,
    gas_value: u64,
) -> Result<()> {
    let deploy_salt = canonical_interchain_token_deploy_salt(&ctx.accounts.token_mint.key());
    let token_id = interchain_token_id_internal(&deploy_salt);

    msg!("Instruction: OutboundCanonicalDeploy");

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
        minter: vec![], // Canonical tokens don't have destination minters
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
        minter: Bytes::new(), // Canonical tokens don't have destination minters
    });

    let gmp_accounts = ctx.accounts.to_gmp_accounts();

    process_outbound(gmp_accounts, destination_chain, gas_value, inner_payload)?;

    Ok(())
}
