use anchor_lang::prelude::*;

pub mod instructions;
pub use instructions::*;

pub mod state;
pub use state::*;

use solana_axelar_gateway::executable::{ExecutablePayloadEncodingScheme, Message};

use program_utils::ensure_single_feature;

ensure_single_feature!("devnet-amplifier", "stagenet", "testnet", "mainnet");

#[cfg(feature = "devnet-amplifier")]
declare_id!("mem5NJXuxU7b4UJqq6ib8XUjk1Hnp4z2B1szyRZ8bLv");

#[cfg(feature = "stagenet")]
declare_id!("mempfz1SLfPr1zmackMVMgShjkuCGPZ5taN8wAfwreW");

#[cfg(feature = "testnet")]
declare_id!("mempFGXoWNNMMaYGhJoNRMNAp8R3srFeBmKAoeLgSYy");

#[cfg(feature = "mainnet")]
declare_id!("mem1111111111111111111111111111111111111111");

#[program]
pub mod memo {
    use super::*;

    /// Send a memo message cross-chain via Axelar
    pub fn send_memo(
        ctx: Context<SendMemo>,
        destination_chain: String,
        destination_address: String,
        memo: String,
    ) -> Result<()> {
        instructions::send_memo_handler(ctx, destination_chain, destination_address, memo)
    }

    pub fn init(ctx: Context<Init>) -> Result<()> {
        instructions::init_handler(ctx)
    }

    pub fn execute(
        ctx: Context<Execute>,
        message: Message,
        payload: Vec<u8>,
        encoding_scheme: ExecutablePayloadEncodingScheme,
    ) -> Result<()> {
        instructions::execute_handler(ctx, message, payload, encoding_scheme)
    }

    pub fn emit_memo(ctx: Context<EmitMemo>, message: String) -> Result<()> {
        instructions::emit_memo_handler(ctx, message)
    }
}
