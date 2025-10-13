//! Axelar Interchain Token Service program for the Solana blockchain
#![allow(clippy::little_endian_bytes)]
pub mod errors;
pub mod events;
pub mod instructions;
pub mod state;
pub mod utils;

use instructions::*;

use anchor_lang::prelude::*;
use program_utils::ensure_single_feature;

ensure_single_feature!("devnet-amplifier", "stagenet", "testnet", "mainnet");

#[cfg(feature = "devnet-amplifier")]
declare_id!("itsqybuNsChBo3LgVhCWWnTJVJdoVTUJaodmqQcG6z7");
pub const CHAIN_NAME_HASH: [u8; 32] = [
    10, 171, 102, 67, 72, 176, 161, 92, 42, 179, 148, 228, 13, 72, 172, 178, 168, 16, 138, 252, 99,
    222, 187, 187, 25, 30, 121, 52, 235, 103, 11, 169,
]; // keccak256("solana-devnet")

pub(crate) const ITS_HUB_CHAIN_NAME: &str = "axelar";

#[cfg(feature = "stagenet")]
declare_id!("itsediSVCwwKc6UuxfrsEiF8AEuEFk34RFAscPEDEpJ");

#[cfg(feature = "testnet")]
declare_id!("itsZEirFsnRmLejCsRRNZKHqWTzMsKGyYi6Qr962os4");

#[cfg(feature = "mainnet")]
declare_id!("its1111111111111111111111111111111111111111");

pub mod seed_prefixes {
    /// The seed prefix for deriving the ITS root PDA
    pub const ITS_SEED: &[u8] = b"interchain-token-service";

    /// The seed prefix for deriving the token manager PDA
    pub const TOKEN_MANAGER_SEED: &[u8] = b"token-manager";

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
    pub const DEPLOYMENT_APPROVAL_SEED: &[u8] = b"deployment-approval";

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
        params: DeployInterchainTokenData,
    ) -> Result<()> {
        instructions::deploy_interchain_token_handler(ctx, params)
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
        )
    }
}
