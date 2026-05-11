#![cfg(test)]
#![allow(clippy::indexing_slicing)]

mod helpers;
use helpers::*;

use std::collections::BTreeMap;

use mollusk_harness::gateway::generate_random_signer;
use mollusk_harness::{GatewayTestHarness, TestHarness};
use mollusk_svm::result::Check;
use solana_axelar_gateway::{GatewayConfig, SignatureVerificationSessionData, VerifierSetTracker};
use solana_axelar_std::{PayloadType, PublicKey, VerifierSet, U256};

#[test]
fn test_rotate_signers() {
    let harness = GatewayTestHarness::new();

    let config: GatewayConfig = harness
        .get_account_as(&harness.gateway.root)
        .expect("gateway config should exist");

    // Create new verifier set
    let (_, new_compressed_pubkey_1) = generate_random_signer();
    let (_, new_compressed_pubkey_2) = generate_random_signer();

    let mut new_signers = BTreeMap::new();
    new_signers.insert(PublicKey(new_compressed_pubkey_1), 50u128);
    new_signers.insert(PublicKey(new_compressed_pubkey_2), 50u128);

    let new_verifier_set = VerifierSet {
        nonce: 2,
        signers: new_signers,
        quorum: 100,
    };

    let new_verifier_set_hash =
        compute_new_verifier_set_hash(config.domain_separator, &new_verifier_set);

    // Initialize verification session for rotation
    let verification_session_pda = harness
        .init_payload_verification_session(new_verifier_set_hash, PayloadType::RotateSigners);

    // Verify all signatures using harness's own verifier set
    let verifier_infos =
        build_verifier_infos(&harness, new_verifier_set_hash, PayloadType::RotateSigners);
    for info in &verifier_infos {
        harness.verify_signature(new_verifier_set_hash, info.clone());
    }

    // Check the session is valid
    let session: SignatureVerificationSessionData = harness
        .get_account_as(&verification_session_pda)
        .expect("verification session should exist");
    assert!(
        session.signature_verification.is_valid(),
        "Rotation should be approved by both signers"
    );

    // Execute rotation
    harness.rotate_signers(new_verifier_set_hash, verification_session_pda);

    // Verify the new verifier set tracker
    let (new_tracker_pda, _) = VerifierSetTracker::find_pda(&new_verifier_set_hash);
    let new_tracker: VerifierSetTracker = harness
        .get_account_as(&new_tracker_pda)
        .expect("new verifier set tracker should exist");

    assert_eq!(new_tracker.verifier_set_hash, new_verifier_set_hash);
    assert_eq!(
        new_tracker.epoch,
        U256::from(1u64).checked_add(U256::ONE).unwrap()
    );

    // Verify gateway config was updated
    let updated_config: GatewayConfig = harness
        .get_account_as(&harness.gateway.root)
        .expect("gateway config should exist");

    assert_eq!(
        updated_config.current_epoch,
        U256::from(1u64).checked_add(U256::ONE).unwrap()
    );
}

#[test]
fn test_fails_when_using_approve_messages_payload_for_rotate_signers() {
    let harness = GatewayTestHarness::new();

    let config: GatewayConfig = harness
        .get_account_as(&harness.gateway.root)
        .expect("gateway config should exist");

    let messages = default_messages();
    let (_, payload_merkle_root) = create_merklized_messages(config.domain_separator, &messages);

    // Initialize with ApproveMessages payload type
    let verification_session_pda = harness
        .init_payload_verification_session(payload_merkle_root, PayloadType::ApproveMessages);

    // Sign with both signers using ApproveMessages type
    let verifier_infos =
        build_verifier_infos(&harness, payload_merkle_root, PayloadType::ApproveMessages);
    for info in &verifier_infos {
        harness.verify_signature(payload_merkle_root, info.clone());
    }

    // Verify session is valid
    let session: SignatureVerificationSessionData = harness
        .get_account_as(&verification_session_pda)
        .expect("verification session should exist");
    assert!(session.signature_verification.is_valid());

    // Try to use it for rotate_signers -- should fail because the verification
    // session was initialized with ApproveMessages type, not RotateSigners.
    harness.rotate_signers_with_checks(
        payload_merkle_root,
        verification_session_pda,
        &[Check::err({
            let e: anchor_lang::error::Error =
                anchor_lang::error::ErrorCode::ConstraintSeeds.into();
            e.into()
        })],
    );
}

#[test]
fn rotate_signers_duplicate_verifier_set() {
    let harness = GatewayTestHarness::new();

    let tracker: VerifierSetTracker = harness
        .get_account_as(&harness.gateway.verifier_set_tracker)
        .expect("verifier set tracker should exist");

    // The current verifier set hash is already stored. If we try to rotate to
    // the same verifier set hash, it should fail with DuplicateVerifierSetRotation.
    let existing_hash = tracker.verifier_set_hash;

    // Initialize verification session for rotation using the existing hash
    let verification_session_pda =
        harness.init_payload_verification_session(existing_hash, PayloadType::RotateSigners);

    // Verify all signatures
    let verifier_infos = build_verifier_infos(&harness, existing_hash, PayloadType::RotateSigners);
    for info in &verifier_infos {
        harness.verify_signature(existing_hash, info.clone());
    }

    // Attempt rotation to the same verifier set.
    // The DuplicateVerifierSetRotation constraint is on verifier_set_tracker_pda,
    // but because new_verifier_set_tracker uses `init` with the same PDA seeds,
    // the system program rejects the account creation first (account already in use).
    // This results in Custom(0) from the system program.
    harness.rotate_signers_with_checks(
        existing_hash,
        verification_session_pda,
        &[Check::err(solana_sdk::program_error::ProgramError::Custom(
            0,
        ))],
    );
}
