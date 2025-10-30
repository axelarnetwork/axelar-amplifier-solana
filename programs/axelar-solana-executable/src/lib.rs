use anchor_lang::prelude::*;

pub mod instructions;
pub use instructions::*;

pub mod state;
pub use state::*;

use axelar_solana_gateway_v2::executable::{ExecutablePayloadEncodingScheme, Message};
use relayer_discovery::structs::RelayerTransaction;

declare_id!("8VRxuTLvEWsUcGsA299QQdUPaFuYkV6qkHDC5gtqt3Zc");

#[program]
pub mod memo {

    use super::*;

    pub fn init(ctx: Context<Init>) -> Result<()> {
        instructions::init_handler(ctx)
    }

    pub fn get_transaction(ctx: Context<GetTransaction>) -> Result<RelayerTransaction> {
        instructions::get_transaction_handler(ctx)
    }

    pub fn execute(
        ctx: Context<Execute>,
        message: Message,
        payload: Payload,
        encoding_scheme: ExecutablePayloadEncodingScheme,
    ) -> Result<()> {
        instructions::execute_handler(ctx, message, payload, encoding_scheme)
    }
}
