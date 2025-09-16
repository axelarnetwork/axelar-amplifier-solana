use crate::{
    GatewayConfig, GatewayError, IncomingMessage, MerkleisedMessage, MessageApprovedEvent,
    MessageStatus, SignatureVerificationSessionData,
};
use anchor_lang::prelude::*;
use axelar_solana_encoding::{hasher::SolanaSyscallHasher, rs_merkle};
use axelar_solana_gateway::seed_prefixes::{
    GATEWAY_SEED, INCOMING_MESSAGE_SEED, SIGNATURE_VERIFICATION_SEED,
};
use std::str::FromStr;

#[derive(Accounts)]
#[event_cpi]
#[instruction(approve_message_instruction: ApproveMessageInstruction)]
pub struct ApproveMessage<'info> {
    #[account(
            seeds = [GATEWAY_SEED],
            bump = gateway_root_pda.bump
        )]
    pub gateway_root_pda: Account<'info, GatewayConfig>,
    #[account(mut)]
    pub funder: Signer<'info>,
    #[account(
            seeds = [SIGNATURE_VERIFICATION_SEED, approve_message_instruction.payload_merkle_root.as_ref()],
            bump = verification_session_account.bump
        )]
    pub verification_session_account: Account<'info, SignatureVerificationSessionData>,
    #[account(
        init,
        payer = funder,
        space = 8 + std::mem::size_of::<IncomingMessage>(),
        seeds = [INCOMING_MESSAGE_SEED, approve_message_instruction.message.leaf.message.command_id().as_ref()],
        bump
    )]
    pub incoming_message_pda: Account<'info, IncomingMessage>,
    pub system_program: Program<'info, System>,
}

#[derive(Debug, AnchorSerialize, AnchorDeserialize)]
pub struct ApproveMessageInstruction {
    _padding: u8,
    /// The message that's to be approved
    pub message: MerkleisedMessage,
    /// The merkle root of the new message batch
    pub payload_merkle_root: [u8; 32],
}

impl ApproveMessageInstruction {
    pub fn new(message: MerkleisedMessage, payload_merkle_root: [u8; 32]) -> Self {
        Self {
            _padding: 0,
            message,
            payload_merkle_root,
        }
    }
}

pub fn approve_message_handler(
    ctx: Context<ApproveMessage>,
    approve_message_instruction: ApproveMessageInstruction,
) -> Result<()> {
    let message = approve_message_instruction.message;
    let payload_merkle_root = approve_message_instruction.payload_merkle_root;

    msg!("Approving message!");

    let gateway_config = &ctx.accounts.gateway_root_pda;
    let verification_session = &ctx.accounts.verification_session_account;
    let incoming_message_pda = &mut ctx.accounts.incoming_message_pda;

    // Validate signature verification session is complete
    if !verification_session.signature_verification.is_valid() {
        return err!(GatewayError::SigningSessionNotValid);
    }

    // Validate message signing verifier set matches verification session
    if message.leaf.signing_verifier_set
        != verification_session
            .signature_verification
            .signing_verifier_set_hash
    {
        return err!(GatewayError::InvalidVerificationSessionPDA);
    }

    // Validate domain separator matches gateway config
    if message.leaf.domain_separator != gateway_config.domain_separator {
        return err!(GatewayError::InvalidDomainSeparator);
    }

    let leaf_hash = message.leaf.hash();
    let message_hash = message.leaf.message.hash();
    let proof = rs_merkle::MerkleProof::<SolanaSyscallHasher>::from_bytes(&message.proof)
        .map_err(|_err| GatewayError::InvalidMerkleProof)?;

    // Check: leaf node is part of the payload merkle root
    if !proof.verify(
        payload_merkle_root,
        &[message.leaf.position.into()],
        &[leaf_hash],
        message.leaf.set_size.into(),
    ) {
        return err!(GatewayError::LeafNodeNotPartOfMerkleRoot);
    }

    let command_id = message.leaf.message.command_id();

    // Parse destination address
    let destination_address = Pubkey::from_str(&message.leaf.message.destination_address)
        .map_err(|_| GatewayError::InvalidDestinationAddress)?;

    // Calculate signing PDA bump
    let (_, signing_pda_bump) =
        axelar_solana_gateway::get_validate_message_signing_pda(destination_address, command_id);

    // Store data in the PDA
    incoming_message_pda.bump = ctx.bumps.incoming_message_pda;
    incoming_message_pda.signing_pda_bump = signing_pda_bump;
    incoming_message_pda.status = MessageStatus::approved();
    incoming_message_pda.message_hash = message_hash;
    incoming_message_pda.payload_hash = message.leaf.message.payload_hash;

    let cc_id = &message.leaf.message.cc_id;

    emit_cpi!(MessageApprovedEvent {
        command_id,
        destination_address,
        payload_hash: message.leaf.message.payload_hash,
        source_chain: cc_id.chain.clone(),
        message_id: cc_id.id.clone(),
        source_address: message.leaf.message.source_address.clone(),
        destination_chain: message.leaf.message.destination_chain.clone(),
    });

    Ok(())
}
