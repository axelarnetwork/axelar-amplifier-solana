use crate::gmp::{GMPAccounts, ToGMPAccounts};
use crate::instructions::deploy_remote_interchain_token::{get_token_metadata, process_outbound};
use crate::{
    errors::ITSError,
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
use axelar_solana_gas_service_v2::state::Treasury;
use axelar_solana_gateway_v2::{seed_prefixes::CALL_CONTRACT_SIGNING_SEED, GatewayConfig};
use interchain_token_transfer_gmp::{DeployInterchainToken, GMPPayload};

/// Accounts required for deploying a remote canonical interchain token
#[derive(Accounts)]
#[event_cpi]
#[instruction(destination_chain: String, gas_value: u64, signing_pda_bump: u8)]
pub struct DeployRemoteCanonicalInterchainToken<'info> {
    /// The account which is paying for the transaction
    #[account(mut)]
    pub payer: Signer<'info>,

    /// The existing mint account for the canonical token
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

    /// The token manager account associated with the canonical interchain token
    #[account(
        seeds = [
            crate::seed_prefixes::TOKEN_MANAGER_SEED,
            its_root_pda.key().as_ref(),
            &canonical_interchain_token_id(&token_mint.key())
        ],
        seeds::program = crate::ID,
        bump = token_manager_pda.bump,
    )]
    pub token_manager_pda: Account<'info, TokenManager>,

    // GMP Accounts
    /// The GMP gateway root account
    #[account(
        seeds = [
            axelar_solana_gateway_v2::seed_prefixes::GATEWAY_SEED
        ],
        seeds::program = axelar_solana_gateway_v2::ID,
        bump = gateway_root_pda.load()?.bump,
    )]
    pub gateway_root_pda: AccountLoader<'info, GatewayConfig>,

    /// The GMP gateway program account
    #[account(address = axelar_solana_gateway_v2::ID)]
    pub axelar_gateway_program: AccountInfo<'info>,

    /// The GMP gas treasury account
    #[account(
        mut,
        seeds = [Treasury::SEED_PREFIX],
        seeds::program = axelar_solana_gas_service_v2::ID,
        bump = gas_treasury.bump,
    )]
    pub gas_treasury: Account<'info, Treasury>,

    /// The GMP gas service program account
    #[account(address = axelar_solana_gas_service_v2::ID)]
    pub gas_service: AccountInfo<'info>,

    /// The system program account
    pub system_program: Program<'info, System>,

    /// The ITS root account
    #[account(
        seeds = [InterchainTokenService::SEED_PREFIX],
        bump = its_root_pda.bump,
        constraint = !its_root_pda.paused @ ITSError::Paused
    )]
    pub its_root_pda: Account<'info, InterchainTokenService>,

    /// The GMP call contract signing account
    #[account(
        seeds = [CALL_CONTRACT_SIGNING_SEED],
        bump = signing_pda_bump,
        seeds::program = crate::ID
    )]
    pub call_contract_signing_pda: Signer<'info>,

    /// The ITS program account (this program)
    #[account(address = crate::ID)]
    pub its_program: AccountInfo<'info>,

    /// Event authority - derived from gateway program
    #[account(
        seeds = [b"__event_authority"],
        bump,
        seeds::program = axelar_gateway_program.key()
    )]
    pub gateway_event_authority: SystemAccount<'info>,

    /// Event authority for gas service - derived from gas service program
    #[account(
        seeds = [b"__event_authority"],
        bump,
        seeds::program = gas_service.key()
    )]
    pub gas_event_authority: SystemAccount<'info>,
}

impl<'info> ToGMPAccounts<'info> for DeployRemoteCanonicalInterchainToken<'info> {
    fn to_gmp_accounts(&self) -> GMPAccounts<'info> {
        GMPAccounts {
            payer: self.payer.to_account_info(),
            gateway_root_pda: self.gateway_root_pda.to_account_info(),
            axelar_gateway_program: self.axelar_gateway_program.clone(),
            gas_treasury: self.gas_treasury.to_account_info(),
            gas_service: self.gas_service.clone(),
            system_program: self.system_program.to_account_info(),
            its_root_pda: self.its_root_pda.clone(),
            call_contract_signing_pda: self.call_contract_signing_pda.to_account_info(),
            its_program: self.its_program.clone(),
            gateway_event_authority: self.gateway_event_authority.to_account_info(),
            gas_event_authority: self.gas_event_authority.to_account_info(),
        }
    }
}

/// Instruction handler for deploying a remote canonical interchain token
pub fn deploy_remote_canonical_interchain_token_handler(
    ctx: Context<DeployRemoteCanonicalInterchainToken>,
    destination_chain: String,
    gas_value: u64,
    signing_pda_bump: u8,
) -> Result<()> {
    let deploy_salt = canonical_interchain_token_deploy_salt(&ctx.accounts.token_mint.key());
    let token_id = interchain_token_id_internal(&deploy_salt);

    if destination_chain == ctx.accounts.its_root_pda.chain_name {
        msg!("Cannot deploy remotely to the origin chain");
        return err!(ITSError::InvalidInstructionData);
    }

    msg!("Instruction: OutboundCanonicalDeploy");

    // get token metadata
    let (name, symbol) = get_token_metadata(
        &ctx.accounts.token_mint.to_account_info(),
        Some(&ctx.accounts.metadata_account),
    )?;
    let decimals = ctx.accounts.token_mint.decimals;

    if ctx.accounts.token_manager_pda.token_address != ctx.accounts.token_mint.key() {
        msg!("TokenManager doesn't match mint");
        return err!(ITSError::InvalidArgument);
    }

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
    process_outbound(
        gmp_accounts,
        destination_chain,
        gas_value,
        signing_pda_bump,
        inner_payload,
    )?;

    Ok(())
}
