use anchor_lang::{prelude::*, solana_program};
use axelar_solana_gateway::seed_prefixes::INCOMING_MESSAGE_SEED;
use axelar_solana_gateway_v2::{
    cpi::accounts::ValidateMessage, program::AxelarSolanaGatewayV2, IncomingMessage, Message,
};

#[error_code]
pub enum ExecutableError {
    InvalidPayloadHash,
}

#[derive(Accounts)]
#[instruction(message: Message)]
pub struct Execute<'info> {
    #[account(
        seeds = [INCOMING_MESSAGE_SEED, message.command_id().as_ref()],
        bump = incoming_message_pda.bump,
        seeds::program = axelar_gateway_program.key()
    )]
    pub incoming_message_pda: Account<'info, IncomingMessage>,

    /// Signing PDA for this program - used to validate with gateway
    #[account(
           mut,
           signer,
           seeds = [message.command_id().as_ref()],
           bump = incoming_message_pda.signing_pda_bump,
       )]
    pub signing_pda: AccountInfo<'info>,

    /// Reference to the axelar gateway program
    pub axelar_gateway_program: Program<'info, AxelarSolanaGatewayV2>,

    /// for event_cpi
    /// Event authority - derived from gateway program
    #[account(
            seeds = [b"__event_authority"],
            bump,
            seeds::program = axelar_gateway_program.key()
        )]
    pub event_authority: SystemAccount<'info>,

    pub system_program: Program<'info, System>,
}

pub fn execute_handler(
    ctx: Context<Execute>,
    message: Message,
    _source_chain: String,
    _source_address: String,
    payload: Vec<u8>,
) -> Result<()> {
    msg!("Executing payload of: {} bytes", payload.len());

    // Check that provided payload matches the approved message
    let compute_payload_hash = solana_program::keccak::hashv(&[&payload]).to_bytes();
    if compute_payload_hash != message.payload_hash {
        return err!(ExecutableError::InvalidPayloadHash);
    }

    let cpi_accounts = ValidateMessage {
        incoming_message_pda: ctx.accounts.incoming_message_pda.to_account_info(),
        caller: ctx.accounts.signing_pda.to_account_info(),
        // for emit cpi
        event_authority: ctx.accounts.event_authority.to_account_info(),
        program: ctx.accounts.axelar_gateway_program.to_account_info(),
    };

    let cpi_ctx = CpiContext::new(
        ctx.accounts.axelar_gateway_program.to_account_info(),
        cpi_accounts,
    );

    axelar_solana_gateway_v2::cpi::validate_message(cpi_ctx, message.clone())?;

    msg!("Message validated successfully!");

    // todo: do something with the message
    Ok(())
}
