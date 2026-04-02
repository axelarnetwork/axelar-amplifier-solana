#![cfg(test)]
#![allow(clippy::indexing_slicing)]

mod helpers;
use helpers::*;

use anchor_lang::{InstructionData, ToAccountMetas};
use mollusk_harness::{GatewayTestHarness, TestHarness};
use mollusk_svm::result::Check;
use solana_axelar_gateway::{
    GatewayConfig, GatewayError, IncomingMessage, MessageStatus, ValidateMessageSigner,
};
use solana_axelar_std::{Message, PayloadType};
use solana_sdk::pubkey::Pubkey;

/// Approves the first default message and returns (message, incoming_message_pda).
fn approve_first_message(harness: &GatewayTestHarness) -> (Message, Pubkey) {
    let config: GatewayConfig = harness
        .get_account_as(&harness.gateway.root)
        .expect("gateway config should exist");

    let messages = default_messages();
    let (merklized_messages, payload_merkle_root) =
        create_merklized_messages(config.domain_separator, &messages);

    let verification_session_pda = harness
        .init_payload_verification_session(payload_merkle_root, PayloadType::ApproveMessages);

    let verifier_infos =
        build_verifier_infos(harness, payload_merkle_root, PayloadType::ApproveMessages);
    for info in &verifier_infos {
        harness.verify_signature(payload_merkle_root, info.clone());
    }

    harness.approve_message(
        &merklized_messages[0],
        payload_merkle_root,
        verification_session_pda,
    );

    let incoming_message_pda = IncomingMessage::find_pda(&messages[0].command_id()).0;
    (messages[0].clone(), incoming_message_pda)
}

/// Builds a ValidateMessage instruction for the given message.
/// The `caller` is set to the signing PDA derived from the message's destination address.
fn build_validate_message_ix(
    harness: &GatewayTestHarness,
    message: &Message,
    incoming_message_pda: Pubkey,
) -> solana_sdk::instruction::Instruction {
    let incoming_message: IncomingMessage = harness
        .get_account_as(&incoming_message_pda)
        .expect("incoming message should exist");

    let destination_address: Pubkey = message.destination_address.parse().unwrap();
    let command_id = message.command_id();

    let caller = ValidateMessageSigner::create_pda(
        &command_id,
        incoming_message.signing_pda_bump,
        &destination_address,
    )
    .expect("valid signing PDA");

    let (event_authority, _, _) =
        mollusk_test_utils::get_event_authority_and_program_accounts(&solana_axelar_gateway::ID);

    solana_sdk::instruction::Instruction {
        program_id: solana_axelar_gateway::ID,
        accounts: solana_axelar_gateway::accounts::ValidateMessage {
            incoming_message_pda,
            caller,
            gateway_root_pda: harness.gateway.root,
            event_authority,
            program: solana_axelar_gateway::ID,
        }
        .to_account_metas(None),
        data: solana_axelar_gateway::instruction::ValidateMessage {
            message: message.clone(),
        }
        .data(),
    }
}

#[test]
fn validate_message_success() {
    let harness = GatewayTestHarness::new();

    // Step 1: Approve a message via the full flow
    let (message, incoming_message_pda) = approve_first_message(&harness);

    // Verify it's in approved state
    let incoming: IncomingMessage = harness
        .get_account_as(&incoming_message_pda)
        .expect("incoming message should exist");
    assert_eq!(incoming.status, MessageStatus::approved());

    // Step 2: Validate (execute) the message
    let ix = build_validate_message_ix(&harness, &message, incoming_message_pda);

    harness
        .ctx
        .process_and_validate_instruction_chain(&[(&ix, &[Check::success()])]);

    // Step 3: Verify the message is now executed
    let incoming_after: IncomingMessage = harness
        .get_account_as(&incoming_message_pda)
        .expect("incoming message should still exist");
    assert_eq!(incoming_after.status, MessageStatus::executed());
}

#[test]
fn validate_message_already_executed() {
    let harness = GatewayTestHarness::new();

    // Approve and validate a message
    let (message, incoming_message_pda) = approve_first_message(&harness);

    let ix = build_validate_message_ix(&harness, &message, incoming_message_pda);

    // First validation should succeed
    harness
        .ctx
        .process_and_validate_instruction_chain(&[(&ix, &[Check::success()])]);

    // Second validation should fail because it's already executed
    let ix2 = build_validate_message_ix(&harness, &message, incoming_message_pda);
    harness.ctx.process_and_validate_instruction_chain(&[(
        &ix2,
        &[Check::err(gateway_err(GatewayError::MessageNotApproved))],
    )]);
}

#[test]
fn validate_message_wrong_hash() {
    let harness = GatewayTestHarness::new();

    // Approve the first message
    let (message, incoming_message_pda) = approve_first_message(&harness);

    // Create a modified message with a different payload_hash but the same cc_id
    // (so it maps to the same incoming_message_pda, but the hash check should fail)
    let wrong_message = Message {
        cc_id: message.cc_id.clone(),
        source_address: message.source_address.clone(),
        destination_chain: message.destination_chain.clone(),
        destination_address: message.destination_address.clone(),
        payload_hash: [99u8; 32], // different payload hash
    };

    // The wrong message has a different command_id hash, but we need the same command_id
    // for the PDA to match. Since command_id is derived from cc_id, the PDA is the same.
    // However, the message.hash() will differ, which triggers InvalidMessageHash.
    let incoming_message: IncomingMessage = harness
        .get_account_as(&incoming_message_pda)
        .expect("incoming message should exist");

    let destination_address: Pubkey = wrong_message.destination_address.parse().unwrap();
    let command_id = wrong_message.command_id();

    let caller = ValidateMessageSigner::create_pda(
        &command_id,
        incoming_message.signing_pda_bump,
        &destination_address,
    )
    .expect("valid signing PDA");

    let (event_authority, _, _) =
        mollusk_test_utils::get_event_authority_and_program_accounts(&solana_axelar_gateway::ID);

    let ix = solana_sdk::instruction::Instruction {
        program_id: solana_axelar_gateway::ID,
        accounts: solana_axelar_gateway::accounts::ValidateMessage {
            incoming_message_pda,
            caller,
            gateway_root_pda: harness.gateway.root,
            event_authority,
            program: solana_axelar_gateway::ID,
        }
        .to_account_metas(None),
        data: solana_axelar_gateway::instruction::ValidateMessage {
            message: wrong_message,
        }
        .data(),
    };

    harness.ctx.process_and_validate_instruction_chain(&[(
        &ix,
        &[Check::err(gateway_err(GatewayError::InvalidMessageHash))],
    )]);
}

#[test]
fn validate_message_wrong_caller_pda() {
    let harness = GatewayTestHarness::new();

    let (message, incoming_message_pda) = approve_first_message(&harness);

    let incoming_message: IncomingMessage = harness
        .get_account_as(&incoming_message_pda)
        .expect("incoming message should exist");

    // Derive a signing PDA from a DIFFERENT program (not the destination)
    let wrong_program = Pubkey::new_unique();
    let command_id = message.command_id();
    let wrong_caller = ValidateMessageSigner::create_pda(
        &command_id,
        incoming_message.signing_pda_bump,
        &wrong_program,
    )
    .expect("valid signing PDA");

    let (event_authority, _, _) =
        mollusk_test_utils::get_event_authority_and_program_accounts(&solana_axelar_gateway::ID);

    let ix = solana_sdk::instruction::Instruction {
        program_id: solana_axelar_gateway::ID,
        accounts: solana_axelar_gateway::accounts::ValidateMessage {
            incoming_message_pda,
            caller: wrong_caller,
            gateway_root_pda: harness.gateway.root,
            event_authority,
            program: solana_axelar_gateway::ID,
        }
        .to_account_metas(None),
        data: solana_axelar_gateway::instruction::ValidateMessage {
            message: message.clone(),
        }
        .data(),
    };

    harness.ctx.process_and_validate_instruction_chain(&[(
        &ix,
        &[Check::err(gateway_err(GatewayError::InvalidSigningPDA))],
    )]);
}
