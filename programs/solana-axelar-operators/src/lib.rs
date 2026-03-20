//! Axelar Operators program for the Solana blockchain
// Anchor's #[program] macro generates code using deprecated AccountInfo::realloc
#![allow(deprecated)]

use anchor_lang::prelude::*;

pub mod errors;
pub mod events;
pub mod instructions;
pub mod state;

pub use errors::ErrorCode;
use instructions::*;
pub use state::*;

use solana_axelar_std::ensure_single_feature;

solana_axelar_std::ensure_single_feature!("devnet-amplifier", "stagenet", "testnet", "mainnet");

#[cfg(feature = "devnet-amplifier")]
declare_id!("oprVNGMBsXzJJBTDQasNWqQ8nZqNhJP2ZXvrC7b5xXd");

#[cfg(feature = "stagenet")]
declare_id!("oprNyqaxdvxs9s3Ngyi1gHqXeM2QLTv8skVPCcqmekx");

#[cfg(feature = "testnet")]
declare_id!("opriTiaaV7Ew1kma71TjKbrwQ2RWKJ3wbiWEAp3hUZc");

#[cfg(feature = "mainnet")]
declare_id!("opr1111111111111111111111111111111111111111");

#[program]
pub mod operators {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        instructions::initialize(ctx)
    }

    pub fn add_operator(ctx: Context<AddOperator>) -> Result<()> {
        instructions::add_operator(ctx)
    }

    pub fn remove_operator(ctx: Context<RemoveOperator>) -> Result<()> {
        instructions::remove_operator(ctx)
    }

    pub fn transfer_owner(ctx: Context<TransferOwner>) -> Result<()> {
        instructions::transfer_owner(ctx)
    }
}
