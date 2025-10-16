//! Axelar Interchain Token Service program for the Solana blockchain
#![allow(clippy::little_endian_bytes)]
pub mod events;
pub mod instructions;
pub mod state;

use instructions::*;

use anchor_lang::prelude::*;
use program_utils::ensure_single_feature;

ensure_single_feature!("devnet-amplifier", "stagenet", "testnet", "mainnet");

#[cfg(feature = "devnet-amplifier")]
declare_id!("itsqybuNsChBo3LgVhCWWnTJVJdoVTUJaodmqQcG6z7");

#[cfg(feature = "stagenet")]
declare_id!("itsediSVCwwKc6UuxfrsEiF8AEuEFk34RFAscPEDEpJ");

#[cfg(feature = "testnet")]
declare_id!("itsZEirFsnRmLejCsRRNZKHqWTzMsKGyYi6Qr962os4");

#[cfg(feature = "mainnet")]
declare_id!("its1111111111111111111111111111111111111111");

/// Discriminators for the top-level instructions supported by the Axelar ITS program.
/// These discriminators are inherited from the v1 program to maintain backwards compatibility.
pub struct Discriminators;

impl Discriminators {
    pub const INITIALIZE: &'static [u8] = &[0];
    pub const SET_PAUSE_STATUS: &'static [u8] = &[1];
    pub const SET_TRUSTED_CHAIN: &'static [u8] = &[2];
    pub const REMOVE_TRUSTED_CHAIN: &'static [u8] = &[3];
}

#[program]
pub mod axelar_solana_its_v2 {
    use super::*;

    #[instruction(discriminator = Discriminators::INITIALIZE)]
    pub fn initialize(
        ctx: Context<Initialize>,
        chain_name: String,
        its_hub_address: String,
    ) -> Result<()> {
        instructions::initialize::initialize(ctx, chain_name, its_hub_address)
    }

    #[instruction(discriminator = Discriminators::SET_PAUSE_STATUS)]
    pub fn set_pause_status(ctx: Context<SetPauseStatus>, paused: bool) -> Result<()> {
        instructions::set_pause_status::set_pause_status(ctx, paused)
    }

    #[instruction(discriminator = Discriminators::SET_TRUSTED_CHAIN)]
    pub fn set_trusted_chain(ctx: Context<SetTrustedChain>, chain_name: String) -> Result<()> {
        instructions::set_trusted_chain::set_trusted_chain(ctx, chain_name)
    }

    #[instruction(discriminator = Discriminators::REMOVE_TRUSTED_CHAIN)]
    pub fn remove_trusted_chain(
        ctx: Context<RemoveTrustedChain>,
        chain_name: String,
    ) -> Result<()> {
        instructions::remove_trusted_chain::remove_trusted_chain(ctx, chain_name)
    }
}
