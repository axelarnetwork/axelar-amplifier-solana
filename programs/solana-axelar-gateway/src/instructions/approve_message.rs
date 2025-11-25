use crate::seed_prefixes::VALIDATE_MESSAGE_SIGNING_SEED;
use crate::{
    GatewayConfig, GatewayError, IncomingMessage, MessageApprovedEvent, MessageStatus,
    SignatureVerificationSessionData,
};
use anchor_lang::prelude::*;
use solana_axelar_std::hasher::LeafHash;
use solana_axelar_std::{MerklizedMessage, PayloadType};
use std::str::FromStr;

#[derive(Accounts)]
#[event_cpi]
#[instruction(merklized_message: MerklizedMessage, payload_merkle_root: [u8; 32])]
pub struct ApproveMessage<'info> {
    #[account(
        seeds = [GatewayConfig::SEED_PREFIX],
        bump = gateway_root_pda.load()?.bump
    )]
    pub gateway_root_pda: AccountLoader<'info, GatewayConfig>,

    #[account(mut)]
    pub funder: Signer<'info>,

    #[account(
        seeds = [
            SignatureVerificationSessionData::SEED_PREFIX,
            payload_merkle_root.as_ref(),
            &[PayloadType::ApproveMessages as u8],
            verification_session_account.load()?.signature_verification.signing_verifier_set_hash.as_ref()
        ],
        bump = verification_session_account.load()?.bump,
        // CHECK: Validate signature verification session is complete
        constraint = verification_session_account.load()?.is_valid() @ GatewayError::SigningSessionNotValid
    )]
    pub verification_session_account: AccountLoader<'info, SignatureVerificationSessionData>,

    #[account(
        init,
        payer = funder,
        space = IncomingMessage::DISCRIMINATOR.len() + std::mem::size_of::<IncomingMessage>(),
        seeds = [IncomingMessage::SEED_PREFIX, merklized_message.leaf.message.command_id().as_ref()],
        bump
    )]
    pub incoming_message_pda: AccountLoader<'info, IncomingMessage>,

    pub system_program: Program<'info, System>,
}

pub fn approve_message_handler(
    ctx: Context<ApproveMessage>,
    merklized_message: MerklizedMessage,
    payload_merkle_root: [u8; 32],
) -> Result<()> {
    msg!("Approving message!");

    let gateway_config = &ctx.accounts.gateway_root_pda.load()?;
    let incoming_message_pda = &mut ctx.accounts.incoming_message_pda.load_init()?;

    // Validate domain separator matches gateway config
    if merklized_message.leaf.domain_separator != gateway_config.domain_separator {
        return err!(GatewayError::InvalidDomainSeparator);
    }

    let leaf_hash = merklized_message.leaf.hash();
    let message_hash = merklized_message.leaf.message.hash();
    let proof = solana_axelar_std::MerkleProof::from_bytes(&merklized_message.proof)
        .map_err(|_err| GatewayError::InvalidMerkleProof)?;

    // Check: leaf node is part of the payload merkle root
    if !proof.verify(
        payload_merkle_root,
        &[merklized_message.leaf.position.into()],
        &[leaf_hash],
        merklized_message.leaf.set_size.into(),
    ) {
        return err!(GatewayError::LeafNodeNotPartOfMerkleRoot);
    }

    let command_id = merklized_message.leaf.message.command_id();

    // Parse destination address
    let destination_address = Pubkey::from_str(&merklized_message.leaf.message.destination_address)
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
    incoming_message_pda.payload_hash = merklized_message.leaf.message.payload_hash;

    let cc_id = &merklized_message.leaf.message.cc_id;

    emit_cpi!(MessageApprovedEvent {
        command_id,
        destination_address,
        payload_hash: merklized_message.leaf.message.payload_hash,
        source_chain: cc_id.chain.clone(),
        cc_id: cc_id.id.clone(),
        source_address: merklized_message.leaf.message.source_address.clone(),
        destination_chain: merklized_message.leaf.message.destination_chain.clone(),
    });

    Ok(())
}
