use anchor_lang::prelude::*;

pub mod instructions;
pub use instructions::*;

pub mod state;
pub use state::*;

use relayer_discovery::structs::RelayerTransaction;
use solana_axelar_gateway::executable::Message;

declare_id!("8VRxuTLvEWsUcGsA299QQdUPaFuYkV6qkHDC5gtqt3Zc");

#[program]
pub mod executable {

    use super::*;

    pub fn init(ctx: Context<Init>) -> Result<()> {
        instructions::init_handler(ctx)
    }

    pub fn get_transaction(
        ctx: Context<GetTransaction>,
        payload: Payload,
        command_id: [u8; 32],
    ) -> Result<RelayerTransaction> {
        instructions::get_transaction_handler(ctx, payload, command_id)
    }

    pub fn execute(ctx: Context<Execute>, payload: Payload, message: Message) -> Result<()> {
        instructions::execute_handler(ctx, payload, message)
    }
}
