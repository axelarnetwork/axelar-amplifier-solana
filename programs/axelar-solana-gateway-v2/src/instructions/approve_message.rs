use crate::{
    GatewayConfig, GatewayError, IncomingMessage, MerkleisedMessage, MessageApprovedEvent,
    MessageStatus, SignatureVerificationSessionData,
};
use anchor_lang::prelude::*;
use axelar_solana_encoding::{hasher::SolanaSyscallHasher, rs_merkle};
use axelar_solana_gateway::seed_prefixes::{
    GATEWAY_SEED, INCOMING_MESSAGE_SEED, SIGNATURE_VERIFICATION_SEED, VALIDATE_MESSAGE_SIGNING_SEED,
};
use std::str::FromStr;

#[derive(Accounts)]
#[event_cpi]
#[instruction(merkleised_message: MerkleisedMessage, payload_merkle_root: [u8; 32])]
pub struct ApproveMessage<'info> {
    #[account(
        seeds = [GATEWAY_SEED],
        bump = gateway_root_pda.bump
    )]
    pub gateway_root_pda: Account<'info, GatewayConfig>,

    #[account(mut)]
    pub funder: Signer<'info>,

    #[account(
        seeds = [SIGNATURE_VERIFICATION_SEED, payload_merkle_root.as_ref()],
        bump = verification_session_account.bump
    )]
    pub verification_session_account: Account<'info, SignatureVerificationSessionData>,

    #[account(
        init,
        payer = funder,
        space = IncomingMessage::DISCRIMINATOR.len() + std::mem::size_of::<IncomingMessage>(),
        seeds = [INCOMING_MESSAGE_SEED, merkleised_message.leaf.message.command_id().as_ref()],
        bump
    )]
    pub incoming_message_pda: Account<'info, IncomingMessage>,

    pub system_program: Program<'info, System>,
}

pub fn approve_message_handler(
    ctx: Context<ApproveMessage>,
    merkleised_message: MerkleisedMessage,
    payload_merkle_root: [u8; 32],
) -> Result<()> {
    msg!("Approving message!");

    let gateway_config = &ctx.accounts.gateway_root_pda;
    let verification_session = &ctx.accounts.verification_session_account;
    let incoming_message_pda = &mut ctx.accounts.incoming_message_pda;

    // Validate signature verification session is complete
    if !verification_session.signature_verification.is_valid() {
        return err!(GatewayError::SigningSessionNotValid);
    }

    // Validate message signing verifier set matches verification session
    if merkleised_message.leaf.signing_verifier_set
        != verification_session
            .signature_verification
            .signing_verifier_set_hash
    {
        return err!(GatewayError::InvalidVerificationSessionPDA);
    }

    // Validate domain separator matches gateway config
    if merkleised_message.leaf.domain_separator != gateway_config.domain_separator {
        return err!(GatewayError::InvalidDomainSeparator);
    }

    let leaf_hash = merkleised_message.leaf.hash();
    let message_hash = merkleised_message.leaf.message.hash();
    let proof =
        rs_merkle::MerkleProof::<SolanaSyscallHasher>::from_bytes(&merkleised_message.proof)
            .map_err(|_err| GatewayError::InvalidMerkleProof)?;

    // Check: leaf node is part of the payload merkle root
    if !proof.verify(
        payload_merkle_root,
        &[merkleised_message.leaf.position.into()],
        &[leaf_hash],
        merkleised_message.leaf.set_size.into(),
    ) {
        return err!(GatewayError::LeafNodeNotPartOfMerkleRoot);
    }

    let command_id = merkleised_message.leaf.message.command_id();

    // Parse destination address
    let destination_address =
        Pubkey::from_str(&merkleised_message.leaf.message.destination_address)
            .map_err(|_| GatewayError::InvalidDestinationAddress)?;

    // Create a new Signing PDA that is used for validating that a message has
    // reached the destination program
    //
    // Calculate signing PDA bump
    let (_, signing_pda_bump) = Pubkey::find_program_address(
        &[VALIDATE_MESSAGE_SIGNING_SEED, command_id.as_ref()],
        &destination_address,
    );

    // Store data in the PDA
    incoming_message_pda.bump = ctx.bumps.incoming_message_pda;
    incoming_message_pda.signing_pda_bump = signing_pda_bump;
    incoming_message_pda.status = MessageStatus::approved();
    incoming_message_pda.message_hash = message_hash;
    incoming_message_pda.payload_hash = merkleised_message.leaf.message.payload_hash;

    let cc_id = &merkleised_message.leaf.message.cc_id;

    emit_cpi!(MessageApprovedEvent {
        command_id,
        destination_address,
        payload_hash: merkleised_message.leaf.message.payload_hash,
        source_chain: cc_id.chain.clone(),
        cc_id: cc_id.id.clone(),
        source_address: merkleised_message.leaf.message.source_address.clone(),
        destination_chain: merkleised_message.leaf.message.destination_chain.clone(),
    });

    Ok(())
}
