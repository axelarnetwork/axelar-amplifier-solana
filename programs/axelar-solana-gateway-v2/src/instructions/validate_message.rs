use crate::{GatewayError, IncomingMessage, Message, MessageExecutedEvent, MessageStatus};
use anchor_lang::prelude::*;
use axelar_solana_gateway::seed_prefixes::{INCOMING_MESSAGE_SEED, VALIDATE_MESSAGE_SIGNING_SEED};
use std::str::FromStr;

#[derive(Accounts)]
#[event_cpi]
#[instruction(message: Message)]
pub struct ValidateMessage<'info> {
    #[account(
        seeds = [INCOMING_MESSAGE_SEED, message.command_id().as_ref()],
        bump = incoming_message_pda.bump,
        constraint = incoming_message_pda.status.is_approved() @ GatewayError::MessageNotApproved,
                constraint = incoming_message_pda.message_hash == message.hash() @ GatewayError::InvalidMessageHash
    )]
    pub incoming_message_pda: Account<'info, IncomingMessage>,
    /// The caller must be a PDA derived from the destination program using command_id and signing_pda_bump
    #[account(
        mut,
        signer,
        constraint = validate_caller_pda(&caller, &message, &incoming_message_pda)? @ GatewayError::InvalidSigningPDA
    )]
    pub caller: AccountInfo<'info>,
}

pub fn validate_message_handler(ctx: Context<ValidateMessage>, message: Message) -> Result<()> {
    ctx.accounts.incoming_message_pda.status = MessageStatus::executed();

    // Parse destination address
    let destination_address = Pubkey::from_str(&message.destination_address)
        .map_err(|_| GatewayError::InvalidDestinationAddress)?;

    let command_id = message.command_id();
    let cc_id = &message.cc_id;

    emit_cpi!(MessageExecutedEvent {
        command_id,
        destination_address,
        payload_hash: message.payload_hash,
        source_chain: cc_id.chain.clone(),
        cc_id: cc_id.id.clone(),
        source_address: message.source_address.clone(),
        destination_chain: message.destination_chain.clone(),
    });

    Ok(())
}

fn validate_caller_pda(
    caller: &AccountInfo,
    message: &Message,
    incoming_message: &IncomingMessage,
) -> Result<bool> {
    use std::str::FromStr;

    let destination_address = Pubkey::from_str(&message.destination_address)
        .map_err(|_| GatewayError::InvalidDestinationAddress)?;

    let command_id = message.command_id();

    // Pubkey::create_program_address(&[prefix, command_id, &[signing_pda_bump]], destination_address)
    // each message has its own signing pda for a given executable
    let expected_signing_pda = Pubkey::create_program_address(
        &[
            VALIDATE_MESSAGE_SIGNING_SEED,
            command_id.as_ref(),
            &[incoming_message.signing_pda_bump],
        ],
        &destination_address,
    )
    .map_err(|_| GatewayError::InvalidSigningPDA)?;

    Ok(caller.key == &expected_signing_pda)
}
