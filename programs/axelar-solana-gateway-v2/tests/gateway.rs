use anchor_lang::AccountDeserialize;
use axelar_solana_gateway_v2::u256::U256;
use axelar_solana_gateway_v2::{
    signature_verification::{SignatureVerification, VerificationSessionAccount},
    state::VerifierSetTracker,
    GatewayConfig, ID as GATEWAY_PROGRAM_ID,
};
use axelar_solana_gateway_v2::{IncomingMessage, MessageStatus};
use axelar_solana_gateway_v2_test_fixtures::{
    approve_message_helper, create_verifier_info, initialize_gateway,
    initialize_payload_verification_session, initialize_payload_verification_session_with_root,
    mock_setup_test, rotate_signers_helper, setup_message_merkle_tree,
    setup_signer_rotation_payload, setup_test_with_real_signers, transfer_operatorship_helper,
    verify_signature_helper,
};
use solana_sdk::pubkey::Pubkey;

#[test]
fn test_initialize_config() {
    let gateway_caller = None;
    let setup = mock_setup_test(gateway_caller);
    let result = initialize_gateway(&setup);

    assert!(!result.program_result.is_err());

    // Test the gateway config account
    let gateway_account = result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == setup.gateway_root_pda)
        .unwrap()
        .1
        .clone();

    let expected_config = GatewayConfig {
        current_epoch: setup.epoch,
        previous_verifier_set_retention: setup.previous_verifier_retention,
        minimum_rotation_delay: setup.minimum_rotation_delay,
        last_rotation_timestamp: 0,
        operator: setup.operator,
        domain_separator: setup.domain_separator,
        bump: setup.gateway_bump,
    };

    let actual_config =
        GatewayConfig::try_deserialize(&mut gateway_account.data.as_slice()).unwrap();

    assert_eq!(actual_config, expected_config);

    let verifier_set_account = result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == setup.verifier_set_tracker_pda)
        .unwrap()
        .1
        .clone();

    let actual_verifier_set_tracker =
        VerifierSetTracker::try_deserialize(&mut verifier_set_account.data.as_slice()).unwrap();

    let expected_verifier_set_tracker = VerifierSetTracker {
        bump: setup.verifier_bump,
        verifier_set_hash: setup.verifier_set_hash,
        epoch: setup.epoch,
    };

    assert_eq!(expected_verifier_set_tracker, actual_verifier_set_tracker);
}

#[test]
fn test_initialize_payload_verification_session() {
    let gateway_caller = None;
    let setup = mock_setup_test(gateway_caller);

    let init_result = initialize_gateway(&setup);
    assert!(!init_result.program_result.is_err());

    let (result, verification_session_pda) =
        initialize_payload_verification_session(&setup, &init_result);

    assert!(
        !result.program_result.is_err(),
        "Instruction should succeed: {:?}",
        result.program_result
    );

    let verification_account = result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == verification_session_pda)
        .unwrap()
        .1
        .clone();

    let actual_verification_account =
        VerificationSessionAccount::try_deserialize(&mut verification_account.data.as_slice())
            .unwrap();

    let expected_verification_account = VerificationSessionAccount {
        signature_verification: SignatureVerification {
            accumulated_threshold: 0,
            signature_slots: [0u8; 32],
            signing_verifier_set_hash: [0u8; 32],
        },
        bump: 255, // we know the bump in this case since the seed is static
    };

    assert_eq!(expected_verification_account, actual_verification_account);
}

#[test]
fn test_approve_message_with_dual_signers_and_merkle_proof() {
    // Step 1: Setup gateway with real signers
    let (setup, verifier_leaves, verifier_merkle_tree, secret_key_1, secret_key_2) =
        setup_test_with_real_signers();

    // Step 2: Initialize gateway
    let init_result = initialize_gateway(&setup);
    assert!(!init_result.program_result.is_err());

    // Step 3: Create messages and payload merkle root
    let verifier_set_merkle_root = setup.verifier_set_hash;
    let (messages, message_leaves, message_merkle_tree, payload_merkle_root) =
        setup_message_merkle_tree(&setup, verifier_set_merkle_root);

    // Step 4: Initialize payload verification session
    let (session_result, verification_session_pda) =
        initialize_payload_verification_session_with_root(
            &setup,
            &init_result,
            payload_merkle_root,
        );
    assert!(!session_result.program_result.is_err());

    // Step 5: Get existing accounts
    let gateway_account = init_result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == setup.gateway_root_pda)
        .unwrap()
        .1
        .clone();

    let verifier_set_tracker_account = init_result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == setup.verifier_set_tracker_pda)
        .unwrap()
        .1
        .clone();

    let verification_session_account = session_result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == verification_session_pda)
        .unwrap()
        .1
        .clone();

    // Step 6: Sign the payload with both signers and verify signatures
    // Create verifier info for first signer
    let verifier_info_1 = create_verifier_info(
        &secret_key_1,
        payload_merkle_root,
        &verifier_leaves[0],
        0, // Position 0
        &verifier_merkle_tree,
    );

    // Execute the first verify_signature instruction using helper
    let verify_result_1 = verify_signature_helper(
        &setup,
        payload_merkle_root,
        verifier_info_1,
        verification_session_pda,
        gateway_account.clone(),
        verification_session_account.clone(),
        setup.verifier_set_tracker_pda,
        verifier_set_tracker_account.clone(),
    );

    assert!(
        !verify_result_1.program_result.is_err(),
        "First signature verification should succeed: {:?}",
        verify_result_1.program_result
    );

    // Get updated verification session after first signature
    let updated_verification_account_after_first = verify_result_1
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == verification_session_pda)
        .unwrap()
        .1
        .clone();

    // Create verifier info for second signer
    let verifier_info_2 = create_verifier_info(
        &secret_key_2,
        payload_merkle_root,
        &verifier_leaves[1],
        1, // Position 1
        &verifier_merkle_tree,
    );

    // Execute the second verify_signature instruction using helper
    let verify_result_2 = verify_signature_helper(
        &setup,
        payload_merkle_root,
        verifier_info_2,
        verification_session_pda,
        gateway_account,
        updated_verification_account_after_first,
        setup.verifier_set_tracker_pda,
        verifier_set_tracker_account,
    );

    assert!(
        !verify_result_2.program_result.is_err(),
        "Second signature verification should succeed: {:?}",
        verify_result_2.program_result
    );

    // Step 7: Check the session contents to verify quorum was reached
    let final_verification_account = verify_result_2
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == verification_session_pda)
        .unwrap()
        .1
        .clone();

    let final_verification_session = VerificationSessionAccount::try_deserialize(
        &mut final_verification_account.data.as_slice(),
    )
    .unwrap();

    assert!(
        final_verification_session.signature_verification.is_valid(),
        "Accumulated threshold should equal 100% after both signatures"
    );

    assert_eq!(
        final_verification_session
            .signature_verification
            .signature_slots,
        [
            3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0
        ],
        "Signature slots should show positions 0 and 1 are verified (3 = 0b11)"
    );

    assert_eq!(
        final_verification_session
            .signature_verification
            .signing_verifier_set_hash,
        verifier_set_merkle_root,
        "Signing verifier set hash should match our merkle root"
    );

    let (approve_result, incoming_message_pda) = approve_message_helper(
        &setup,
        message_merkle_tree,
        message_leaves,
        &messages,
        payload_merkle_root,
        verification_session_pda,
        verify_result_2,
        0, // position
    );

    assert!(
        !approve_result.program_result.is_err(),
        "Approve message instruction should succeed: {:?}",
        approve_result.program_result
    );

    let incoming_message_account = approve_result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == incoming_message_pda)
        .unwrap()
        .1
        .clone();

    assert_eq!(
        incoming_message_account.owner, GATEWAY_PROGRAM_ID,
        "Incoming message account should be owned by gateway program"
    );

    let incoming_message_account_data =
        IncomingMessage::try_deserialize(&mut incoming_message_account.data.as_slice()).unwrap();

    assert_eq!(
        incoming_message_account_data.message_hash,
        messages[0].hash()
    );

    assert_eq!(
        incoming_message_account_data.status,
        MessageStatus::approved()
    );
}

#[test]
fn test_rotate_signers() {
    // Step 1: Setup gateway with real signers (current verifier set)
    let (setup, verifier_leaves, verifier_merkle_tree, secret_key_1, secret_key_2) =
        setup_test_with_real_signers();

    // Step 2: Initialize gateway
    let init_result = initialize_gateway(&setup);
    assert!(!init_result.program_result.is_err());

    // Step 3: Create new verifier set that we want to rotate to
    let new_verifier_set_hash = [42u8; 32];

    // Step 4: Create rotation payload hash (what current verifiers need to sign)
    let current_verifier_set_hash = setup.verifier_set_hash;
    let rotation_payload_hash =
        setup_signer_rotation_payload(current_verifier_set_hash, new_verifier_set_hash);

    // Step 5: Initialize payload verification session (for the rotation)
    let (session_result, verification_session_pda) =
        initialize_payload_verification_session_with_root(
            &setup,
            &init_result,
            rotation_payload_hash,
        );
    assert!(!session_result.program_result.is_err());

    // Step 6: Get existing accounts
    let gateway_account = init_result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == setup.gateway_root_pda)
        .unwrap()
        .1
        .clone();

    let verifier_set_tracker_account = init_result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == setup.verifier_set_tracker_pda)
        .unwrap()
        .1
        .clone();

    let verification_session_account = session_result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == verification_session_pda)
        .unwrap()
        .1
        .clone();

    // Step 7: CURRENT verifiers sign the ROTATION payload

    // First verifier signs for rotation
    let verifier_info_1 = create_verifier_info(
        &secret_key_1,
        rotation_payload_hash,
        &verifier_leaves[0],
        0,
        &verifier_merkle_tree,
    );

    let verify_result_1 = verify_signature_helper(
        &setup,
        rotation_payload_hash,
        verifier_info_1,
        verification_session_pda,
        gateway_account.clone(),
        verification_session_account.clone(),
        setup.verifier_set_tracker_pda,
        verifier_set_tracker_account.clone(),
    );

    assert!(!verify_result_1.program_result.is_err());

    // Get updated verification session after first signature
    let updated_verification_account_after_first = verify_result_1
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == verification_session_pda)
        .unwrap()
        .1
        .clone();

    // Second verifier signs for rotation
    let verifier_info_2 = create_verifier_info(
        &secret_key_2,
        rotation_payload_hash,
        &verifier_leaves[1],
        1,
        &verifier_merkle_tree,
    );

    let verify_result_2 = verify_signature_helper(
        &setup,
        rotation_payload_hash,
        verifier_info_2,
        verification_session_pda,
        gateway_account.clone(),
        updated_verification_account_after_first,
        setup.verifier_set_tracker_pda,
        verifier_set_tracker_account,
    );

    assert!(!verify_result_2.program_result.is_err());

    // Step 8: Verify the session is complete
    let final_verification_account = verify_result_2
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == verification_session_pda)
        .unwrap()
        .1
        .clone();

    let final_verification_session = VerificationSessionAccount::try_deserialize(
        &mut final_verification_account.data.as_slice(),
    )
    .unwrap();

    assert!(
        final_verification_session.signature_verification.is_valid(),
        "Rotation should be approved by both signers"
    );

    // Step 9: Execute the rotation instruction
    let rotate_result = rotate_signers_helper(
        &setup,
        new_verifier_set_hash,
        verification_session_pda,
        verify_result_2,
    );

    assert!(
        !rotate_result.program_result.is_err(),
        "Rotation instruction should succeed: {:?}",
        rotate_result.program_result
    );

    // Step 10: Verify the new verifier set tracker was created correctly
    let (new_verifier_set_tracker_pda, _) = Pubkey::find_program_address(
        &[
            axelar_solana_gateway::seed_prefixes::VERIFIER_SET_TRACKER_SEED,
            new_verifier_set_hash.as_slice(),
        ],
        &GATEWAY_PROGRAM_ID,
    );

    let new_verifier_set_account = rotate_result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == new_verifier_set_tracker_pda)
        .unwrap()
        .1
        .clone();

    let new_tracker =
        VerifierSetTracker::try_deserialize(&mut new_verifier_set_account.data.as_slice()).unwrap();

    assert_eq!(new_tracker.verifier_set_hash, new_verifier_set_hash);
    assert_eq!(new_tracker.epoch, setup.epoch + U256::ONE);

    // Step 11: Verify gateway config was updated
    let updated_gateway_account = rotate_result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == setup.gateway_root_pda)
        .unwrap()
        .1
        .clone();

    let updated_config =
        GatewayConfig::try_deserialize(&mut updated_gateway_account.data.as_slice()).unwrap();

    assert_eq!(updated_config.current_epoch, setup.epoch + U256::ONE);
}

#[test]
fn test_transfer_operatorship() {
    let gateway_caller = None;
    let setup = mock_setup_test(gateway_caller);

    // Initialize gateway first
    let init_result = initialize_gateway(&setup);
    assert!(!init_result.program_result.is_err());

    // Create a new operator
    let new_operator = Pubkey::new_unique();

    let result = transfer_operatorship_helper(&setup, init_result, new_operator);

    assert!(
        !result.program_result.is_err(),
        "Transfer operatorship should succeed: {:?}",
        result.program_result
    );

    // Verify that the operator was changed
    let updated_gateway_account = result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == setup.gateway_root_pda)
        .unwrap()
        .1
        .clone();

    let updated_config =
        GatewayConfig::try_deserialize(&mut updated_gateway_account.data.as_slice()).unwrap();
    assert_eq!(updated_config.operator, new_operator);
}
