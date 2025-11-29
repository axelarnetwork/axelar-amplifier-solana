use anchor_lang::prelude::*;

pub mod instructions;
pub use instructions::*;

pub mod state;
pub use state::*;

use relayer_discovery::structs::RelayerTransaction;
use solana_axelar_gateway::executable::Message;
use solana_axelar_its::executable::AxelarExecuteWithInterchainTokenPayload;

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

    pub fn get_its_transaction(
        ctx: Context<GetItsTransaction>,
        message: Message,
        payload: Vec<u8>,
    ) -> Result<RelayerTransaction> {
        instructions::get_its_transaction_handler(ctx, message, payload)
    }

    pub fn execute(ctx: Context<Execute>, payload: Payload, message: Message) -> Result<()> {
        instructions::execute_handler(ctx, payload, message)
    }

    pub fn execute_with_interchain_token(
        ctx: Context<ExecuteWithInterchainToken>,
        execute_payload: AxelarExecuteWithInterchainTokenPayload,
    ) -> Result<()> {
        instructions::execute_with_interchain_token_handler(ctx, execute_payload)
    }
}
