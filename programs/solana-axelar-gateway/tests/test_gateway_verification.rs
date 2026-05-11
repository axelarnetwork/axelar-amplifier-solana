#![cfg(test)]
#![allow(clippy::indexing_slicing)]

mod helpers;
use helpers::*;

use mollusk_harness::{GatewayTestHarness, TestHarness};
use mollusk_svm::result::Check;
use solana_axelar_gateway::{
    GatewayConfig, GatewayError, IncomingMessage, MessageStatus, SignatureVerificationSessionData,
    VerifierSetTracker,
};
use solana_axelar_std::hasher::LeafHash;
use solana_axelar_std::PayloadType;

#[test]
fn initialize_payload_verification_session_works() {
    let harness = GatewayTestHarness::new();

    let payload_merkle_root = [2u8; 32];
    let payload_type = PayloadType::ApproveMessages;

    let verification_session_pda =
        harness.init_payload_verification_session(payload_merkle_root, payload_type);

    let tracker: VerifierSetTracker = harness
        .get_account_as(&harness.gateway.verifier_set_tracker)
        .expect("verifier set tracker should exist");

    let (_, bump) = SignatureVerificationSessionData::find_pda(
        &payload_merkle_root,
        payload_type,
        &tracker.verifier_set_hash,
    );

    let actual: SignatureVerificationSessionData = harness
        .get_account_as(&verification_session_pda)
        .expect("verification session should exist");

    assert_eq!(actual.bump, bump);
    assert_eq!(
        actual.signature_verification.signing_verifier_set_hash,
        tracker.verifier_set_hash,
    );
}

#[test]
fn test_approve_message_with_dual_signers_and_merkle_proof() {
    let harness = GatewayTestHarness::new();

    let config: GatewayConfig = harness
        .get_account_as(&harness.gateway.root)
        .expect("gateway config should exist");

    let messages = default_messages();
    let (merklized_messages, payload_merkle_root) =
        create_merklized_messages(config.domain_separator, &messages);
    let payload_type = PayloadType::ApproveMessages;

    // Initialize verification session
    let verification_session_pda =
        harness.init_payload_verification_session(payload_merkle_root, payload_type);

    // Verify signatures from both signers using the harness's own verifier set
    let verifier_infos = build_verifier_infos(&harness, payload_merkle_root, payload_type);
    for info in &verifier_infos {
        harness.verify_signature(payload_merkle_root, info.clone());
    }

    // Check the session is valid
    let session: SignatureVerificationSessionData = harness
        .get_account_as(&verification_session_pda)
        .expect("verification session should exist");

    assert!(
        session.signature_verification.is_valid(),
        "Accumulated threshold should equal 100% after both signatures"
    );
    assert_eq!(
        session.signature_verification.signature_slots,
        [
            3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0
        ],
        "Signature slots should show positions 0 and 1 are verified (3 = 0b11)"
    );

    let tracker: VerifierSetTracker = harness
        .get_account_as(&harness.gateway.verifier_set_tracker)
        .expect("verifier set tracker should exist");
    assert_eq!(
        session.signature_verification.signing_verifier_set_hash,
        tracker.verifier_set_hash,
    );

    // Approve the first message
    harness.approve_message(
        &merklized_messages[0],
        payload_merkle_root,
        verification_session_pda,
    );

    let incoming_message_pda = IncomingMessage::find_pda(&messages[0].command_id()).0;

    let incoming_message_account = harness
        .get_account(&incoming_message_pda)
        .expect("incoming message account should exist");

    assert_eq!(
        incoming_message_account.owner,
        solana_axelar_gateway::ID,
        "Incoming message account should be owned by gateway program"
    );

    let incoming_message: IncomingMessage = harness
        .get_account_as(&incoming_message_pda)
        .expect("incoming message should deserialize");

    assert_eq!(incoming_message.message_hash, messages[0].hash());
    assert_eq!(incoming_message.status, MessageStatus::approved());
}

#[test]
fn test_fails_when_verifier_submits_signature_twice() {
    let harness = GatewayTestHarness::new();

    let config: GatewayConfig = harness
        .get_account_as(&harness.gateway.root)
        .expect("gateway config should exist");

    let messages = default_messages();
    let (_, payload_merkle_root) = create_merklized_messages(config.domain_separator, &messages);
    let payload_type = PayloadType::ApproveMessages;

    harness.init_payload_verification_session(payload_merkle_root, payload_type);

    let verifier_infos = build_verifier_infos(&harness, payload_merkle_root, payload_type);
    let verifier_info = verifier_infos[0].clone();

    // First verification should succeed
    harness.verify_signature(payload_merkle_root, verifier_info.clone());

    // Second verification with the same verifier should fail
    harness.verify_signature_with_checks(
        payload_merkle_root,
        verifier_info,
        &[Check::err(gateway_err(GatewayError::SlotAlreadyVerified))],
    );
}

#[test]
fn test_fails_when_approving_message_with_insufficient_signatures() {
    let harness = GatewayTestHarness::new();

    let config: GatewayConfig = harness
        .get_account_as(&harness.gateway.root)
        .expect("gateway config should exist");

    let messages = default_messages();
    let (merklized_messages, payload_merkle_root) =
        create_merklized_messages(config.domain_separator, &messages);
    let payload_type = PayloadType::ApproveMessages;

    let verification_session_pda =
        harness.init_payload_verification_session(payload_merkle_root, payload_type);

    // Sign with only one signer (insufficient quorum)
    let verifier_infos = build_verifier_infos(&harness, payload_merkle_root, payload_type);
    harness.verify_signature(payload_merkle_root, verifier_infos[0].clone());

    // Attempt to approve with insufficient signatures should fail
    harness.approve_message_with_checks(
        &merklized_messages[0],
        payload_merkle_root,
        verification_session_pda,
        &[Check::err(gateway_err(
            GatewayError::SigningSessionNotValid,
        ))],
    );
}

#[test]
fn test_fails_when_verifying_invalid_signature() {
    let harness = GatewayTestHarness::new();

    let config: GatewayConfig = harness
        .get_account_as(&harness.gateway.root)
        .expect("gateway config should exist");

    // Create real messages for the verification session
    let messages = default_messages();
    let (_, payload_merkle_root) = create_merklized_messages(config.domain_separator, &messages);
    let payload_type = PayloadType::ApproveMessages;

    harness.init_payload_verification_session(payload_merkle_root, payload_type);

    // Create signatures for FAKE messages (wrong payload) -- the signed hash won't match
    let fake = fake_messages();
    let (_, fake_merkle_root) = create_merklized_messages(config.domain_separator, &fake);
    let fake_verifier_infos = build_verifier_infos(&harness, fake_merkle_root, payload_type);

    // Try to verify a signature made over the FAKE payload against the REAL session
    harness.verify_signature_with_checks(
        payload_merkle_root,
        fake_verifier_infos[0].clone(),
        &[Check::err(gateway_err(
            GatewayError::SignatureVerificationFailed,
        ))],
    );
}

#[test]
fn approve_message_invalid_merkle_proof() {
    let harness = GatewayTestHarness::new();

    let config: GatewayConfig = harness
        .get_account_as(&harness.gateway.root)
        .expect("gateway config should exist");

    let messages = default_messages();
    let (merklized_messages, payload_merkle_root) =
        create_merklized_messages(config.domain_separator, &messages);
    let payload_type = PayloadType::ApproveMessages;

    // Initialize and verify all signatures (valid session)
    let verification_session_pda =
        harness.init_payload_verification_session(payload_merkle_root, payload_type);

    let verifier_infos = build_verifier_infos(&harness, payload_merkle_root, payload_type);
    for info in &verifier_infos {
        harness.verify_signature(payload_merkle_root, info.clone());
    }

    // Create a tampered merklized message with a wrong proof (use msg[1]'s proof for msg[0])
    let tampered_message = solana_axelar_std::MerklizedMessage {
        leaf: merklized_messages[0].leaf.clone(),
        proof: merklized_messages[1].proof.clone(), // wrong proof
    };

    harness.approve_message_with_checks(
        &tampered_message,
        payload_merkle_root,
        verification_session_pda,
        &[Check::err(gateway_err(
            GatewayError::LeafNodeNotPartOfMerkleRoot,
        ))],
    );
}
