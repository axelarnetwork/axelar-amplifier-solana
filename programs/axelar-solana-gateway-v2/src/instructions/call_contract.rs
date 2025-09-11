use crate::{CallContractEvent, GatewayConfig, GatewayError};
use anchor_lang::prelude::*;
use anchor_lang::solana_program::keccak;
use axelar_solana_gateway::seed_prefixes::{CALL_CONTRACT_SIGNING_SEED, GATEWAY_SEED};

#[derive(Accounts)]
#[event_cpi]
pub struct CallContract<'info> {
    /// The program that wants to call us - must be executable
    /// CHECK: Anchor constraint verifies this is an executable program
    pub calling_program: UncheckedAccount<'info>,
    /// The standardized PDA that must sign - derived from the calling program
    pub signing_pda: UncheckedAccount<'info>,
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
    signing_pda_bump: u8,
) -> Result<()> {
    let payload_hash = keccak::hash(&payload);
    let signing_pda = &ctx.accounts.signing_pda;
    let sender = &ctx.accounts.calling_program;

    // Check: sender is a program
    if sender.executable {
        // If so, check that signing PDA is valid
        let expected_signing_pda = Pubkey::create_program_address(
            &[CALL_CONTRACT_SIGNING_SEED, &[signing_pda_bump]],
            sender.key,
        )
        .map_err(|_| GatewayError::InvalidSigningPDA)?;

        if &expected_signing_pda != signing_pda.key {
            msg!("Invalid signing PDA");
            return err!(GatewayError::InvalidSigningPDA);
        }

        if !signing_pda.is_signer {
            msg!("Signing PDA must be a signer");
            return err!(GatewayError::CallerNotSigner);
        }
    } else {
        // Otherwise, the sender must be a signer
        if !sender.is_signer {
            msg!("Sender must be a signer or a program + signing PDA");
            return Err(GatewayError::CallerNotSigner.into());
        }
    }

    emit_cpi!(CallContractEvent {
        sender_key: ctx.accounts.signing_pda.key(),
        payload_hash: payload_hash.to_bytes(),
        destination_chain: destination_chain.clone(),
        destination_contract_address: destination_contract_address.clone(),
        payload,
    });

    Ok(())
}
