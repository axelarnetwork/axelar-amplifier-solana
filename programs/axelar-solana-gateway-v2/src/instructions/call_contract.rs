use crate::{CallContractEvent, GatewayConfig};
use anchor_lang::prelude::*;
use anchor_lang::solana_program::keccak;
use axelar_solana_gateway::seed_prefixes::{CALL_CONTRACT_SIGNING_SEED, GATEWAY_SEED};

#[derive(Accounts)]
#[event_cpi]
pub struct CallContract<'info> {
    /// The program that wants to call us - must be executable
    /// CHECK: Anchor constraint verifies this is an executable program
    #[account(executable)]
    pub calling_program: UncheckedAccount<'info>,

    /// The standardized PDA that must sign - derived from the calling program
    #[account(
        seeds = [CALL_CONTRACT_SIGNING_SEED],
        bump,
        seeds::program = calling_program.key()
    )]
    pub signing_pda: Signer<'info>,
    /// The gateway configuration PDA being initialized
    #[account(
            seeds = [GATEWAY_SEED],
            bump = gateway_root_pda.bump
        )]
    pub gateway_root_pda: Account<'info, GatewayConfig>,
}

pub fn call_contract_handler(
    ctx: Context<CallContract>,
    destination_chain: String,
    destination_contract_address: String,
    payload: Vec<u8>,
) -> Result<()> {
    let payload_hash = keccak::hash(&payload);

    emit_cpi!(CallContractEvent {
        sender_key: ctx.accounts.signing_pda.key(),
        payload_hash: payload_hash.to_bytes(),
        destination_chain: destination_chain.clone(),
        destination_contract_address: destination_contract_address.clone(),
        payload,
    });

    Ok(())
}
