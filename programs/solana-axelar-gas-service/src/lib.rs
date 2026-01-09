//! Axelar Gas Service program for the Solana blockchain
#![allow(
    deprecated,
    reason = "event_cpi macro internally still uses AccountInfo which is deprecated"
)]
#![allow(
    clippy::diverging_sub_expression,
    reason = "Anchor generates such code"
)]
pub mod events;
pub mod instructions;
pub mod state;

use instructions::*;
pub use state::*;

use anchor_lang::prelude::*;

// Export current sdk types for downstream users building with a different sdk
// version.
use solana_axelar_std::ensure_single_feature;

solana_axelar_std::ensure_single_feature!("devnet-amplifier", "stagenet", "testnet", "mainnet");

#[cfg(feature = "devnet-amplifier")]
declare_id!("gas5H48imY2w1j6e6x3EvVxKFtyTuWFhw6pgS5VAPLu");

#[cfg(feature = "stagenet")]
declare_id!("gaspfz1SLfPr1zmackMVMgShjkuCGPZ5taN8wAfwreW");

#[cfg(feature = "testnet")]
declare_id!("gaspFGXoWNNMMaYGhJoNRMNAp8R3srFeBmKAoeLgSYy");

#[cfg(feature = "mainnet")]
declare_id!("gas1111111111111111111111111111111111111111");

#[program]
pub mod solana_axelar_gas_service {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        instructions::initialize::initialize(ctx)
    }

    //
    // Gas-related operations with native token SOL
    //

    pub fn pay_gas(
        ctx: Context<PayGas>,
        destination_chain: String,
        destination_address: String,
        payload_hash: [u8; 32],
        amount: u64,
        refund_address: Pubkey,
    ) -> Result<()> {
        instructions::pay_gas::pay_gas(
            ctx,
            destination_chain,
            destination_address,
            payload_hash,
            amount,
            refund_address,
        )
    }

    pub fn add_gas(
        ctx: Context<AddGas>,
        message_id: String,
        amount: u64,
        refund_address: Pubkey,
    ) -> Result<()> {
        instructions::add_gas::add_gas(ctx, message_id, amount, refund_address)
    }

    pub fn collect_fees(ctx: Context<CollectFees>, amount: u64) -> Result<()> {
        instructions::collect_fees::collect_native_fees(ctx, amount)
    }

    pub fn refund_fees(ctx: Context<RefundFees>, message_id: String, amount: u64) -> Result<()> {
        instructions::refund_fees::refund_native_fees(ctx, message_id, amount)
    }
}
