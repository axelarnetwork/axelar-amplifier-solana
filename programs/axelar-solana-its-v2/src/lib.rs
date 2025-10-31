//! Axelar Gas Service program for the Solana blockchain
#![allow(clippy::little_endian_bytes)]
pub mod errors;
pub mod events;
pub mod gmp;
pub mod instructions;
pub mod state;
pub mod utils;

use instructions::*;

use anchor_lang::prelude::*;
use program_utils::ensure_single_feature;

pub(crate) const ITS_HUB_CHAIN_NAME: &str = "axelar";

ensure_single_feature!("devnet-amplifier", "stagenet", "testnet", "mainnet");

// Program ID

#[cfg(feature = "devnet-amplifier")]
declare_id!("itsqybuNsChBo3LgVhCWWnTJVJdoVTUJaodmqQcG6z7");

#[cfg(feature = "stagenet")]
declare_id!("itsediSVCwwKc6UuxfrsEiF8AEuEFk34RFAscPEDEpJ");

#[cfg(feature = "testnet")]
declare_id!("itsZEirFsnRmLejCsRRNZKHqWTzMsKGyYi6Qr962os4");

#[cfg(feature = "mainnet")]
declare_id!("its1111111111111111111111111111111111111111");

// Chain name hash

// Chain name hash constants for token ID derivation
#[cfg(feature = "devnet-amplifier")]
pub const CHAIN_NAME_HASH: [u8; 32] = [
    10, 171, 102, 67, 72, 176, 161, 92, 42, 179, 148, 228, 13, 72, 172, 178, 168, 16, 138, 252, 99,
    222, 187, 187, 25, 30, 121, 52, 235, 103, 11, 169,
]; // keccak256("solana-devnet")

#[cfg(feature = "stagenet")]
pub const CHAIN_NAME_HASH: [u8; 32] = [
    67, 5, 100, 18, 3, 83, 80, 76, 10, 94, 7, 166, 63, 92, 244, 200, 233, 32, 8, 242, 33, 188, 46,
    11, 38, 32, 244, 151, 37, 161, 40, 0,
]; // keccak256("solana-stagenet")

#[cfg(feature = "testnet")]
pub const CHAIN_NAME_HASH: [u8; 32] = [
    159, 1, 245, 195, 103, 184, 207, 215, 88, 74, 183, 125, 33, 47, 221, 82, 55, 77, 255, 177, 89,
    88, 76, 133, 128, 193, 177, 171, 2, 107, 173, 86,
]; // keccak256("solana-testnet")

#[cfg(feature = "mainnet")]
pub const CHAIN_NAME_HASH: [u8; 32] = [
    110, 239, 41, 235, 176, 58, 162, 20, 74, 26, 107, 98, 18, 206, 116, 245, 4, 163, 77, 183, 153,
    184, 22, 26, 33, 20, 0, 23, 232, 13, 61, 138,
]; // keccak256("solana")

pub mod seed_prefixes {
    use crate::state;

    /// The seed prefix for deriving the ITS root PDA
    pub const ITS_SEED: &[u8] = state::InterchainTokenService::SEED_PREFIX;

    /// The seed prefix for deriving the token manager PDA
    pub const TOKEN_MANAGER_SEED: &[u8] = state::TokenManager::SEED_PREFIX;

    /// The seed prefix for deriving the interchain token PDA
    pub const INTERCHAIN_TOKEN_SEED: &[u8] = b"interchain-token";

    /// The seed prefix for deriving an interchain token id
    pub const PREFIX_INTERCHAIN_TOKEN_ID: &[u8] = b"interchain-token-id";

    /// The seed prefix for deriving an interchain token salt
    pub const PREFIX_INTERCHAIN_TOKEN_SALT: &[u8] = b"interchain-token-salt";

    /// The seed prefix for deriving an interchain token id for a canonical token
    pub const PREFIX_CANONICAL_TOKEN_SALT: &[u8] = b"canonical-token-salt";

    /// The seed prefix for deriving an interchain token id for a canonical token
    pub const PREFIX_CUSTOM_TOKEN_SALT: &[u8] = b"solana-custom-token-salt";

    /// The seed prefix for deriving the flow slot PDA
    pub const FLOW_SLOT_SEED: &[u8] = b"flow-slot";

    /// The seed prefix for deriving the deployment approval PDA
    pub const DEPLOYMENT_APPROVAL_SEED: &[u8] = state::DeployApproval::SEED_PREFIX;

    /// The seed prefix for deriving the interchain transfer execute signing PDA
    pub const INTERCHAIN_TRANSFER_EXECUTE_SEED: &[u8] = b"interchain-transfer-execute";
}

#[program]
pub mod axelar_solana_its_v2 {
    use super::*;

    pub fn initialize(
        ctx: Context<Initialize>,
        chain_name: String,
        its_hub_address: String,
    ) -> Result<()> {
        instructions::initialize::initialize(ctx, chain_name, its_hub_address)
    }

    pub fn set_pause_status(ctx: Context<SetPauseStatus>, paused: bool) -> Result<()> {
        instructions::set_pause_status::set_pause_status(ctx, paused)
    }

    pub fn set_trusted_chain(ctx: Context<SetTrustedChain>, chain_name: String) -> Result<()> {
        instructions::set_trusted_chain::set_trusted_chain(ctx, chain_name)
    }

    pub fn remove_trusted_chain(
        ctx: Context<RemoveTrustedChain>,
        chain_name: String,
    ) -> Result<()> {
        instructions::remove_trusted_chain::remove_trusted_chain(ctx, chain_name)
    }

    pub fn deploy_interchain_token(
        ctx: Context<DeployInterchainToken>,
        salt: [u8; 32],
        name: String,
        symbol: String,
        decimals: u8,
        initial_supply: u64,
    ) -> Result<[u8; 32]> {
        instructions::deploy_interchain_token_handler(
            ctx,
            salt,
            name,
            symbol,
            decimals,
            initial_supply,
        )
    }

    pub fn deploy_remote_interchain_token(
        ctx: Context<DeployRemoteInterchainToken>,
        salt: [u8; 32],
        destination_chain: String,
        gas_value: u64,
        signing_pda_bump: u8,
    ) -> Result<()> {
        instructions::deploy_remote_interchain_token_handler(
            ctx,
            salt,
            destination_chain,
            gas_value,
            signing_pda_bump,
            None,
        )
    }

    pub fn deploy_remote_interchain_token_with_minter(
        ctx: Context<DeployRemoteInterchainToken>,
        salt: [u8; 32],
        destination_chain: String,
        gas_value: u64,
        signing_pda_bump: u8,
        destination_minter: Vec<u8>,
    ) -> Result<()> {
        instructions::deploy_remote_interchain_token_handler(
            ctx,
            salt,
            destination_chain,
            gas_value,
            signing_pda_bump,
            Some(destination_minter),
        )
    }

    pub fn approve_deploy_remote_interchain_token(
        ctx: Context<ApproveDeployRemoteInterchainToken>,
        deployer: Pubkey,
        salt: [u8; 32],
        destination_chain: String,
        destination_minter: Vec<u8>,
    ) -> Result<()> {
        instructions::approve_deploy_remote_interchain_token(
            ctx,
            deployer,
            salt,
            destination_chain,
            destination_minter,
        )
    }

    pub fn revoke_deploy_remote_interchain_token(
        ctx: Context<RevokeDeployRemoteInterchainToken>,
        deployer: Pubkey,
        salt: [u8; 32],
        destination_chain: String,
    ) -> Result<()> {
        instructions::revoke_deploy_remote_interchain_token(ctx, deployer, salt, destination_chain)
    }

    pub fn register_token_metadata(
        ctx: Context<RegisterTokenMetadata>,
        gas_value: u64,
        signing_pda_bump: u8,
    ) -> Result<()> {
        instructions::register_token_metadata_handler(ctx, gas_value, signing_pda_bump)
    }

    pub fn register_canonical_interchain_token(
        ctx: Context<RegisterCanonicalInterchainToken>,
    ) -> Result<[u8; 32]> {
        instructions::register_canonical_interchain_token_handler(ctx)
    }

    pub fn deploy_remote_canonical_interchain_token(
        ctx: Context<DeployRemoteCanonicalInterchainToken>,
        destination_chain: String,
        gas_value: u64,
        signing_pda_bump: u8,
    ) -> Result<()> {
        instructions::deploy_remote_canonical_interchain_token_handler(
            ctx,
            destination_chain,
            gas_value,
            signing_pda_bump,
        )
    }

    pub fn register_custom_token(
        ctx: Context<RegisterCustomToken>,
        salt: [u8; 32],
        token_manager_type: crate::state::Type,
        operator: Option<Pubkey>,
    ) -> Result<[u8; 32]> {
        instructions::register_custom_token_handler(ctx, salt, token_manager_type, operator)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn link_token(
        ctx: Context<LinkToken>,
        salt: [u8; 32],
        destination_chain: String,
        destination_token_address: Vec<u8>,
        token_manager_type: crate::state::Type,
        link_params: Vec<u8>,
        gas_value: u64,
        signing_pda_bump: u8,
    ) -> Result<[u8; 32]> {
        instructions::link_token_handler(
            ctx,
            salt,
            destination_chain,
            destination_token_address,
            token_manager_type,
            link_params,
            gas_value,
            signing_pda_bump,
        )
    }

    pub fn set_flow_limit(ctx: Context<SetFlowLimit>, flow_limit: Option<u64>) -> Result<()> {
        instructions::set_flow_limit_handler(ctx, flow_limit)
    }

    pub fn execute(
        ctx: Context<Execute>,
        message: axelar_solana_gateway_v2::Message,
        payload: Vec<u8>,
    ) -> Result<()> {
        instructions::execute_handler(ctx, message, payload)
    }

    pub fn deploy_interchain_token_internal(
        ctx: Context<DeployInterchainTokenInternal>,
        token_id: [u8; 32],
        name: String,
        symbol: String,
        decimals: u8,
    ) -> Result<()> {
        instructions::deploy_interchain_token_internal_handler(
            ctx, token_id, name, symbol, decimals,
        )
    }

    pub fn link_token_internal(
        ctx: Context<LinkTokenInternal>,
        token_id: [u8; 32],
        destination_token_address: [u8; 32],
        token_manager_type: u8,
        link_params: Vec<u8>,
    ) -> Result<()> {
        instructions::link_token_internal_handler(
            ctx,
            token_id,
            destination_token_address,
            token_manager_type,
            link_params,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn interchain_transfer_internal(
        ctx: Context<InterchainTransferInternal>,
        token_id: [u8; 32],
        source_address: String,
        destination_address: Pubkey,
        amount: u64,
        data: Vec<u8>,
        message: axelar_solana_gateway_v2::Message,
        source_chain: String,
    ) -> Result<()> {
        instructions::interchain_transfer_internal_handler(
            ctx,
            token_id,
            source_address,
            destination_address,
            amount,
            data,
            message,
            source_chain,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn interchain_transfer(
        ctx: Context<InterchainTransfer>,
        token_id: [u8; 32],
        destination_chain: String,
        destination_address: Vec<u8>,
        amount: u64,
        gas_value: u64,
        signing_pda_bump: u8,
        source_id: Option<Pubkey>,
        pda_seeds: Option<Vec<Vec<u8>>>,
        data: Option<Vec<u8>>,
    ) -> Result<()> {
        instructions::interchain_transfer_handler(
            ctx,
            token_id,
            destination_chain,
            destination_address,
            amount,
            gas_value,
            signing_pda_bump,
            source_id,
            pda_seeds,
            data,
        )
    }

    pub fn transfer_operatorship(ctx: Context<TransferOperatorship>) -> Result<()> {
        instructions::transfer_operatorship_handler(ctx)
    }
}
