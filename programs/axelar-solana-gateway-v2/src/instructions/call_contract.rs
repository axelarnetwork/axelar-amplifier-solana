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

#[derive(Debug, AnchorSerialize, AnchorDeserialize)]
pub struct CallContractInstruction {
    _padding: u8,
    /// The name of the target blockchain.
    pub destination_chain: String,
    /// The address of the target contract in the destination blockchain.
    pub destination_contract_address: String,
    /// Contract call data.
    pub payload: Vec<u8>,
    /// The pda bump for the signing PDA
    pub signing_pda_bump: u8,
}

impl CallContractInstruction {
    pub fn new(
        destination_chain: String,
        destination_contract_address: String,
        payload: Vec<u8>,
        signing_pda_bump: u8,
    ) -> Self {
        Self {
            _padding: 0,
            destination_chain,
            destination_contract_address,
            payload,
            signing_pda_bump,
        }
    }
}

pub fn call_contract_handler(
    ctx: Context<CallContract>,
    call_conract_instruction: CallContractInstruction,
) -> Result<()> {
    let destination_chain = call_conract_instruction.destination_chain;
    let destination_contract_address = call_conract_instruction.destination_contract_address;
    let payload = call_conract_instruction.payload;
    let signing_pda_bump = call_conract_instruction.signing_pda_bump;

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
