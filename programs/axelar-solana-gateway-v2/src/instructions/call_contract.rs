use crate::{CallContractEvent, GatewayConfig, GatewayError};
use anchor_lang::prelude::*;
use anchor_lang::solana_program::keccak;
use axelar_solana_gateway::seed_prefixes::{CALL_CONTRACT_SIGNING_SEED, GATEWAY_SEED};

#[derive(Accounts)]
#[event_cpi]
pub struct CallContract<'info> {
    /// The program that wants to call us - can be a direct signer or program
    /// CHECK: We validate the caller using is_signer flag and signing PDA verification
    pub calling_program: UncheckedAccount<'info>,
    /// The standardized PDA that must sign - derived from the calling program
    pub signing_pda: UncheckedAccount<'info>,
    /// The gateway configuration PDA (read-only)
    #[account(
        seeds = [GATEWAY_SEED],
        bump = gateway_root_pda.load()?.bump
    )]
    pub gateway_root_pda: AccountLoader<'info, GatewayConfig>,
}

pub fn call_contract_handler(
    ctx: Context<CallContract>,
    destination_chain: String,
    destination_contract_address: String,
    payload: Vec<u8>,
    signing_pda_bump: u8,
) -> Result<()> {
    let sender = &ctx.accounts.calling_program;
    let signing_pda = &ctx.accounts.signing_pda;

    if sender.is_signer {
        // Direct signer, so not a program, continue
    } else {
        // Case of a program, so a valid signing PDA must be provided
        let Ok(expected_signing_pda) = Pubkey::create_program_address(
            &[CALL_CONTRACT_SIGNING_SEED, &[signing_pda_bump]],
            sender.key,
        ) else {
            msg!("Invalid call: sender must be a direct signer or a valid signing PDA must be provided");
            return err!(GatewayError::CallerNotSigner);
        };

        if &expected_signing_pda != signing_pda.key {
            // Signing PDA mismatch
            msg!("Invalid call: a valid signing PDA must be provided");
            return err!(GatewayError::InvalidSigningPDA);
        }

        if !signing_pda.is_signer {
            // Signing PDA is correct but not a signer
            msg!("Signing PDA must be a signer");
            return err!(GatewayError::CallerNotSigner);
        }

        // A valid signing PDA was provided and it's a signer, continue
    }

    let payload_hash = keccak::hash(&payload);

    emit_cpi!(CallContractEvent {
        sender: ctx.accounts.signing_pda.key(),
        payload_hash: payload_hash.to_bytes(),
        destination_chain,
        destination_contract_address,
        payload,
    });

    Ok(())
}
