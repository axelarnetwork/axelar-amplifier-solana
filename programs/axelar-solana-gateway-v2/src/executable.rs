use anchor_lang::{prelude::*, solana_program};

use crate as axelar_solana_gateway_v2;
use crate::cpi as axelar_solana_gateway_v2_cpi;

use axelar_solana_gateway_v2::{program::AxelarSolanaGatewayV2, seed_prefixes, IncomingMessage};

// Re-export Message
pub use crate::Message;

/// Accounts for executing an inbound Axelar GMP message.
#[derive(Accounts)]
#[instruction(message: Message)]
pub struct AxelarExecuteAccounts<'info> {
    #[account(
        seeds = [IncomingMessage::SEED_PREFIX, message.command_id().as_ref()],
        bump = incoming_message_pda.load()?.bump,
        seeds::program = axelar_gateway_program.key()
    )]
    pub incoming_message_pda: AccountLoader<'info, IncomingMessage>,

    #[account(
        signer,
        seeds = [seed_prefixes::VALIDATE_MESSAGE_SIGNING_SEED, message.command_id().as_ref()],
        bump = incoming_message_pda.load()?.signing_pda_bump,
    )]
    pub signing_pda: AccountInfo<'info>,

    pub axelar_gateway_program: Program<'info, AxelarSolanaGatewayV2>,

    #[account(
        seeds = [b"__event_authority"],
        bump,
        seeds::program = axelar_gateway_program.key()
    )]
    pub event_authority: SystemAccount<'info>,

    pub system_program: Program<'info, System>,
}

pub fn validate_message<'info>(
    executable_accounts: &AxelarExecuteAccounts<'info>,
    message: Message,
    payload: &[u8],
) -> Result<()> {
    let compute_payload_hash = solana_program::keccak::hashv(&[payload]).to_bytes();
    if compute_payload_hash != message.payload_hash {
        return err!(ExecutableError::InvalidPayloadHash);
    }

    let cpi_accounts = axelar_solana_gateway_v2_cpi::accounts::ValidateMessage {
        incoming_message_pda: executable_accounts.incoming_message_pda.to_account_info(),
        caller: executable_accounts.signing_pda.to_account_info(),
        event_authority: executable_accounts.event_authority.to_account_info(),
        program: executable_accounts.axelar_gateway_program.to_account_info(),
    };

    let cpi_ctx = CpiContext::new(
        executable_accounts.axelar_gateway_program.to_account_info(),
        cpi_accounts,
    );

    axelar_solana_gateway_v2_cpi::validate_message(cpi_ctx, message)?;

    msg!("Message validated successfully!");

    Ok(())
}

#[error_code]
pub enum ExecutableError {
    InvalidPayloadHash,
}
