#![cfg(test)]
#![allow(clippy::too_many_lines)]

use std::collections::BTreeMap;

use anchor_lang::{AccountDeserialize, InstructionData, ToAccountMetas};
use solana_axelar_gateway::seed_prefixes::VERIFIER_SET_TRACKER_SEED;
use solana_axelar_gateway::{
    state::VerifierSetTracker, verification_session::SignatureVerification, GatewayConfig,
    ID as GATEWAY_PROGRAM_ID,
};
use solana_axelar_gateway::{IncomingMessage, MessageStatus, SignatureVerificationSessionData};
use solana_axelar_gateway_test_fixtures::{
    approve_message_helper_from_merklized, call_contract_helper,
    create_execute_data_with_signatures, create_merklized_messages_from_std,
    create_signing_verifier_set_leaves, default_messages, fake_messages, generate_random_signer,
    initialize_gateway, initialize_payload_verification_session, mock_setup_test,
    rotate_signers_helper, setup_test_with_real_signers, transfer_operatorship_helper,
    verify_signature_helper,
};
use solana_axelar_std::hasher::LeafHash;
use solana_axelar_std::{Messages, Payload, PayloadType, PublicKey, VerifierSet, U256};
use solana_sdk::{
    account::Account, instruction::Instruction, native_token::LAMPORTS_PER_SOL, pubkey::Pubkey,
};
use solana_sdk_ids::system_program::ID as SYSTEM_PROGRAM_ID;

#[test]
fn initialize_config() {
    let gateway_caller = None;
    let setup = mock_setup_test(gateway_caller);
    let result = initialize_gateway(&setup);

    assert!(!result.program_result.is_err());

    // Test the gateway config account
    let gateway_account = result.get_account(&setup.gateway_root_pda).unwrap().clone();

    let expected_config = GatewayConfig {
        current_epoch: setup.epoch,
        previous_verifier_set_retention: setup.previous_verifier_retention,
        minimum_rotation_delay: setup.minimum_rotation_delay,
        last_rotation_timestamp: 0,
        operator: setup.operator,
        domain_separator: setup.domain_separator,
        bump: setup.gateway_bump,
        _padding: [0u8; 7],
    };

    let actual_config =
        GatewayConfig::try_deserialize(&mut gateway_account.data.as_slice()).unwrap();

    assert_eq!(actual_config, expected_config);

    let verifier_set_account = result
        .get_account(&setup.verifier_set_tracker_pda)
        .unwrap()
        .clone();

    let actual_verifier_set_tracker =
        VerifierSetTracker::try_deserialize(&mut verifier_set_account.data.as_slice()).unwrap();

    let expected_verifier_set_tracker = VerifierSetTracker {
        bump: setup.verifier_bump,
        _padding: [0u8; 7],
        verifier_set_hash: setup.verifier_set_hash,
        epoch: setup.epoch,
    };

    assert_eq!(expected_verifier_set_tracker, actual_verifier_set_tracker);
}

#[test]
fn initialize_payload_verification_session_works() {
    let gateway_caller = None;
    let setup = mock_setup_test(gateway_caller);

    let init_result = initialize_gateway(&setup);
    assert!(!init_result.program_result.is_err());

    let payload_type = PayloadType::ApproveMessages;

    let gateway_account = init_result
        .get_account(&setup.gateway_root_pda)
        .unwrap()
        .clone();
    let verifier_set_tracker_account = init_result
        .get_account(&setup.verifier_set_tracker_pda)
        .unwrap()
        .clone();

    let payload_merkle_root = [2u8; 32];
    let (result, verification_session_pda) = initialize_payload_verification_session(
        &setup,
        gateway_account,
        verifier_set_tracker_account,
        payload_merkle_root,
        payload_type,
    );

    assert!(
        !result.program_result.is_err(),
        "Instruction should succeed: {:?}",
        result.program_result
    );

    let verification_account = result
        .get_account(&verification_session_pda)
        .unwrap()
        .clone();

    let actual_verification_account = SignatureVerificationSessionData::try_deserialize(
        &mut verification_account.data.as_slice(),
    )
    .unwrap();

    let mut expected_verification_account =
        SignatureVerificationSessionData::new(SignatureVerification::default(), 253);

    expected_verification_account
        .signature_verification
        .signing_verifier_set_hash = setup.verifier_set_hash;

    assert_eq!(expected_verification_account, actual_verification_account);
}

#[test]
#[allow(clippy::indexing_slicing)]
fn test_approve_message_with_dual_signers_and_merkle_proof() {
    // Step 1: Setup gateway with real signers
    let (setup, secret_key_1, secret_key_2) = setup_test_with_real_signers();

    // Step 2: Initialize gateway
    let init_result = initialize_gateway(&setup);
    assert!(!init_result.program_result.is_err());

    // Step 3: Create messages and payload merkle root using std crate
    let messages = default_messages();
    let (merklized_messages, payload_merkle_root) =
        create_merklized_messages_from_std(setup.domain_separator, &messages);
    let payload_type = PayloadType::ApproveMessages;

    // Step 4: Initialize payload verification session
    let gateway_account = init_result
        .get_account(&setup.gateway_root_pda)
        .unwrap()
        .clone();
    let verifier_set_tracker_account = init_result
        .get_account(&setup.verifier_set_tracker_pda)
        .unwrap()
        .clone();

    let (session_result, verification_session_pda) = initialize_payload_verification_session(
        &setup,
        gateway_account,
        verifier_set_tracker_account,
        payload_merkle_root,
        payload_type,
    );
    assert!(!session_result.program_result.is_err());

    // Step 5: Get existing accounts
    let gateway_account = init_result
        .get_account(&setup.gateway_root_pda)
        .unwrap()
        .clone();

    let verifier_set_tracker_account = init_result
        .get_account(&setup.verifier_set_tracker_pda)
        .unwrap()
        .clone();

    let verification_session_account = session_result
        .get_account(&verification_session_pda)
        .unwrap()
        .clone();

    let payload_to_be_signed = Payload::Messages(Messages(messages.clone()));
    let signing_verifier_set_leaves = create_signing_verifier_set_leaves(
        setup.domain_separator,
        &secret_key_1,
        &secret_key_2,
        payload_to_be_signed,
        setup.verifier_set.clone(),
    );

    let verifier_info_1 = signing_verifier_set_leaves[0].clone();

    // Execute the first verify_signature instruction using helper
    let verify_result_1 = verify_signature_helper(
        &setup,
        payload_merkle_root,
        verifier_info_1,
        (
            verification_session_pda,
            verification_session_account.clone(),
        ),
        gateway_account.clone(),
        (
            setup.verifier_set_tracker_pda,
            verifier_set_tracker_account.clone(),
        ),
    );

    assert!(
        !verify_result_1.program_result.is_err(),
        "First signature verification should succeed: {:?}",
        verify_result_1.program_result
    );

    // Get updated verification session after first signature
    let updated_verification_account_after_first = verify_result_1
        .get_account(&verification_session_pda)
        .unwrap()
        .clone();

    let verifier_info_2 = signing_verifier_set_leaves[1].clone();

    // Execute the second verify_signature instruction using helper
    let verify_result_2 = verify_signature_helper(
        &setup,
        payload_merkle_root,
        verifier_info_2,
        (
            verification_session_pda,
            updated_verification_account_after_first,
        ),
        gateway_account,
        (setup.verifier_set_tracker_pda, verifier_set_tracker_account),
    );

    assert!(
        !verify_result_2.program_result.is_err(),
        "Second signature verification should succeed: {:?}",
        verify_result_2.program_result
    );

    // Step 7: Check the session contents to verify quorum was reached
    let final_verification_account = verify_result_2
        .get_account(&verification_session_pda)
        .unwrap()
        .clone();

    let final_verification_session = SignatureVerificationSessionData::try_deserialize(
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
        setup.verifier_set_hash,
        "Signing verifier set hash should match our merkle root"
    );

    let final_gateway_account = verify_result_2
        .get_account(&setup.gateway_root_pda)
        .unwrap()
        .clone();
    let final_verification_session_account = verify_result_2
        .get_account(&verification_session_pda)
        .unwrap()
        .clone();

    let (approve_result, incoming_message_pda) = approve_message_helper_from_merklized(
        &setup,
        &merklized_messages[0], // First message
        payload_merkle_root,
        (verification_session_pda, final_verification_session_account),
        final_gateway_account,
    );

    assert!(
        !approve_result.program_result.is_err(),
        "Approve message instruction should succeed: {:?}",
        approve_result.program_result
    );

    let incoming_message_account = approve_result
        .get_account(&incoming_message_pda)
        .unwrap()
        .clone();

    assert_eq!(
        incoming_message_account.owner, GATEWAY_PROGRAM_ID,
        "Incoming message account should be owned by gateway program"
    );

    let incoming_message_account_data =
        IncomingMessage::try_deserialize(&mut incoming_message_account.data.as_slice()).unwrap();

    assert_eq!(
        incoming_message_account_data.message_hash,
        messages.first().unwrap().hash()
    );

    assert_eq!(
        incoming_message_account_data.status,
        MessageStatus::approved()
    );
}

#[test]
#[allow(clippy::indexing_slicing)]
fn test_rotate_signers() {
    // Step 1: Setup gateway with real signers (current verifier set)
    let (setup, secret_key_1, secret_key_2) = setup_test_with_real_signers();

    // Step 2: Initialize gateway
    let init_result = initialize_gateway(&setup);
    assert!(!init_result.program_result.is_err());

    // Step 3: Create new verifier set that we want to rotate to
    // Generate new random signers for the new verifier set
    let (_, new_compressed_pubkey_1) = generate_random_signer();
    let (_, new_compressed_pubkey_2) = generate_random_signer();

    let new_pubkey_1 = PublicKey(new_compressed_pubkey_1);
    let new_pubkey_2 = PublicKey(new_compressed_pubkey_2);

    let mut new_signers = BTreeMap::new();
    new_signers.insert(new_pubkey_1, 50u128);
    new_signers.insert(new_pubkey_2, 50u128);

    let new_verifier_set = VerifierSet {
        nonce: 2, // Next nonce after the current verifier set
        signers: new_signers,
        quorum: 100,
    };

    // Create the payload for the new verifier set
    let new_verifier_set_payload = Payload::NewVerifierSet(new_verifier_set.clone());

    // Generate execute data with signatures from current verifiers
    let execute_data = create_execute_data_with_signatures(
        setup.domain_separator,
        &secret_key_1,
        &secret_key_2,
        new_verifier_set_payload,
        setup.verifier_set.clone(),
    );

    let new_verifier_set_hash = execute_data.payload_merkle_root;

    // Step 4: Create rotation payload hash (what current verifiers need to sign)
    let rotation_payload_hash = new_verifier_set_hash;
    let payload_type = PayloadType::RotateSigners;

    // Step 5: Initialize payload verification session (for the rotation)
    let gateway_account = init_result
        .get_account(&setup.gateway_root_pda)
        .unwrap()
        .clone();
    let verifier_set_tracker_account = init_result
        .get_account(&setup.verifier_set_tracker_pda)
        .unwrap()
        .clone();

    let (session_result, verification_session_pda) = initialize_payload_verification_session(
        &setup,
        gateway_account,
        verifier_set_tracker_account,
        new_verifier_set_hash,
        payload_type,
    );
    assert!(!session_result.program_result.is_err());

    // Step 6: Get existing accounts
    let gateway_account = init_result
        .get_account(&setup.gateway_root_pda)
        .unwrap()
        .clone();

    let verifier_set_tracker_account = init_result
        .get_account(&setup.verifier_set_tracker_pda)
        .unwrap()
        .clone();

    let verification_session_account = session_result
        .get_account(&verification_session_pda)
        .unwrap()
        .clone();

    // Step 7: Use the signing verifier set leaves from execute_data
    let verifier_info_1 = execute_data.signing_verifier_set_leaves[0].clone();

    let verify_result_1 = verify_signature_helper(
        &setup,
        rotation_payload_hash,
        verifier_info_1,
        (
            verification_session_pda,
            verification_session_account.clone(),
        ),
        gateway_account.clone(),
        (
            setup.verifier_set_tracker_pda,
            verifier_set_tracker_account.clone(),
        ),
    );

    assert!(!verify_result_1.program_result.is_err());

    // Get updated verification session after first signature
    let updated_verification_account_after_first = verify_result_1
        .get_account(&verification_session_pda)
        .unwrap()
        .clone();

    // Second verifier signs for rotation
    let verifier_info_2 = execute_data.signing_verifier_set_leaves[1].clone();

    let verify_result_2 = verify_signature_helper(
        &setup,
        rotation_payload_hash,
        verifier_info_2,
        (
            verification_session_pda,
            updated_verification_account_after_first,
        ),
        gateway_account,
        (setup.verifier_set_tracker_pda, verifier_set_tracker_account),
    );

    assert!(!verify_result_2.program_result.is_err());

    // Step 8: Verify the session is complete
    let final_verification_account = verify_result_2
        .get_account(&verification_session_pda)
        .unwrap()
        .clone();

    let final_verification_session = SignatureVerificationSessionData::try_deserialize(
        &mut final_verification_account.data.as_slice(),
    )
    .unwrap();

    assert!(
        final_verification_session.signature_verification.is_valid(),
        "Rotation should be approved by both signers"
    );

    // Step 9: Execute the rotation instruction
    let final_gateway_account = verify_result_2
        .get_account(&setup.gateway_root_pda)
        .unwrap()
        .clone();
    let final_verification_session_account = verify_result_2
        .get_account(&verification_session_pda)
        .unwrap()
        .clone();
    let verifier_set_tracker_account = verify_result_2
        .get_account(&setup.verifier_set_tracker_pda)
        .unwrap()
        .clone();

    let rotate_result = rotate_signers_helper(
        &setup,
        new_verifier_set_hash,
        (verification_session_pda, final_verification_session_account),
        final_gateway_account,
        verifier_set_tracker_account,
    );

    assert!(
        !rotate_result.program_result.is_err(),
        "Rotation instruction should succeed: {:?}",
        rotate_result.program_result
    );

    // Step 10: Verify the new verifier set tracker was created correctly
    let (new_verifier_set_tracker_pda, _) = Pubkey::find_program_address(
        &[VERIFIER_SET_TRACKER_SEED, new_verifier_set_hash.as_slice()],
        &GATEWAY_PROGRAM_ID,
    );

    let new_verifier_set_account = rotate_result
        .get_account(&new_verifier_set_tracker_pda)
        .unwrap()
        .clone();

    let new_tracker =
        VerifierSetTracker::try_deserialize(&mut new_verifier_set_account.data.as_slice()).unwrap();

    assert_eq!(new_tracker.verifier_set_hash, new_verifier_set_hash);
    assert_eq!(
        new_tracker.epoch,
        setup.epoch.checked_add(U256::ONE).unwrap()
    );

    // Step 11: Verify gateway config was updated
    let updated_gateway_account = rotate_result
        .get_account(&setup.gateway_root_pda)
        .unwrap()
        .clone();

    let updated_config =
        GatewayConfig::try_deserialize(&mut updated_gateway_account.data.as_slice()).unwrap();

    assert_eq!(
        updated_config.current_epoch,
        setup.epoch.checked_add(U256::ONE).unwrap()
    );
}

#[test]
fn transfer_operatorship() {
    let gateway_caller = None;
    let setup = mock_setup_test(gateway_caller);

    // Initialize gateway first
    let init_result = initialize_gateway(&setup);
    assert!(!init_result.program_result.is_err());

    // Create a new operator
    let new_operator = Pubkey::new_unique();

    let gateway_account = init_result
        .get_account(&setup.gateway_root_pda)
        .unwrap()
        .clone();
    let program_data_account = init_result
        .get_account(&setup.program_data_pda)
        .unwrap()
        .clone();

    let result =
        transfer_operatorship_helper(&setup, gateway_account, program_data_account, new_operator);

    assert!(
        !result.program_result.is_err(),
        "Transfer operatorship should succeed: {:?}",
        result.program_result
    );

    // Verify that the operator was changed
    let updated_gateway_account = result.get_account(&setup.gateway_root_pda).unwrap().clone();

    let updated_config =
        GatewayConfig::try_deserialize(&mut updated_gateway_account.data.as_slice()).unwrap();
    assert_eq!(updated_config.operator, new_operator);
}

#[test]
fn call_contract_from_program() {
    let memo_program_id = Pubkey::new_unique();
    let setup = mock_setup_test(Some(memo_program_id));

    // Initialize gateway
    let init_result = initialize_gateway(&setup);
    assert!(
        !init_result.program_result.is_err(),
        "Gateway initialization should succeed"
    );

    let gateway_account = init_result
        .get_account(&setup.gateway_root_pda)
        .unwrap()
        .clone();

    let result = call_contract_helper(&setup, gateway_account, memo_program_id);

    assert!(
        !result.program_result.is_err(),
        "call_contract should succeed: {:?}",
        result.program_result
    );
}

#[test]
#[allow(clippy::str_to_string)]
fn call_contract_direct_signer() {
    let setup = mock_setup_test(None);

    // Initialize gateway
    let init_result = initialize_gateway(&setup);
    assert!(
        !init_result.program_result.is_err(),
        "Gateway initialization should succeed"
    );

    let gateway_account = init_result.get_account(&setup.gateway_root_pda).unwrap();

    let (event_authority_pda, _) =
        Pubkey::find_program_address(&[b"__event_authority"], &GATEWAY_PROGRAM_ID);

    // Create a direct signer (e.g., a user wallet)
    let direct_signer = Pubkey::new_unique();

    let destination_chain = "ethereum".to_string();
    let destination_address = "0xDestinationContract".to_string();
    let payload = b"Hello from Solana!".to_vec();
    let signing_pda_bump = 0; // Not used for direct signers

    let call_contract_ix = solana_axelar_gateway::instruction::CallContract {
        destination_chain: destination_chain.clone(),
        destination_contract_address: destination_address.clone(),
        payload: payload.clone(),
        signing_pda_bump,
    };

    let mut accounts = solana_axelar_gateway::accounts::CallContract {
        caller: direct_signer,
        signing_pda: None,
        gateway_root_pda: setup.gateway_root_pda,
        event_authority: event_authority_pda,
        program: GATEWAY_PROGRAM_ID,
    }
    .to_account_metas(None);

    #[allow(clippy::indexing_slicing)]
    {
        accounts[0].is_signer = true; // Mark direct signer as signer
    }

    let instruction = Instruction {
        program_id: GATEWAY_PROGRAM_ID,
        accounts,
        data: call_contract_ix.data(),
    };

    let instruction_accounts = vec![
        (
            direct_signer,
            Account {
                lamports: LAMPORTS_PER_SOL,
                data: vec![],
                owner: SYSTEM_PROGRAM_ID,
                executable: false,
                rent_epoch: 0,
            },
        ),
        (setup.gateway_root_pda, gateway_account.clone()),
        (
            event_authority_pda,
            Account {
                lamports: 0,
                data: vec![],
                owner: SYSTEM_PROGRAM_ID,
                executable: false,
                rent_epoch: 0,
            },
        ),
        (
            GATEWAY_PROGRAM_ID,
            Account {
                lamports: 1,
                data: vec![],
                owner: solana_sdk_ids::bpf_loader_upgradeable::id(),
                executable: true,
                rent_epoch: 0,
            },
        ),
    ];

    let result = setup
        .mollusk
        .process_instruction(&instruction, &instruction_accounts);

    assert!(
        !result.program_result.is_err(),
        "call_contract with direct signer should succeed: {:?}",
        result.program_result
    );
}

#[test]
#[allow(clippy::indexing_slicing)]
fn test_fails_when_verifier_submits_signature_twice() {
    // Setup
    let (setup, secret_key_1, secret_key_2) = setup_test_with_real_signers();

    let init_result = initialize_gateway(&setup);
    assert!(!init_result.program_result.is_err());

    let messages = default_messages();
    let (_, payload_merkle_root) =
        create_merklized_messages_from_std(setup.domain_separator, &messages);

    let payload_type = PayloadType::ApproveMessages;

    let gateway_account = init_result
        .get_account(&setup.gateway_root_pda)
        .unwrap()
        .clone();
    let verifier_set_tracker_account = init_result
        .get_account(&setup.verifier_set_tracker_pda)
        .unwrap()
        .clone();

    let (session_result, verification_session_pda) = initialize_payload_verification_session(
        &setup,
        gateway_account,
        verifier_set_tracker_account,
        payload_merkle_root,
        payload_type,
    );
    assert!(!session_result.program_result.is_err());

    let payload_to_be_signed = Payload::Messages(Messages(messages.clone()));
    let signing_verifier_set_leaves = create_signing_verifier_set_leaves(
        setup.domain_separator,
        &secret_key_1,
        &secret_key_2,
        payload_to_be_signed,
        setup.verifier_set.clone(),
    );

    let verifier_info = signing_verifier_set_leaves[0].clone();

    // First signature verification should succeed
    let verify_result_1 = verify_signature_helper(
        &setup,
        payload_merkle_root,
        verifier_info.clone(),
        (
            verification_session_pda,
            session_result
                .get_account(&verification_session_pda)
                .unwrap()
                .clone(),
        ),
        init_result
            .get_account(&setup.gateway_root_pda)
            .unwrap()
            .clone(),
        (
            setup.verifier_set_tracker_pda,
            init_result
                .get_account(&setup.verifier_set_tracker_pda)
                .unwrap()
                .clone(),
        ),
    );
    assert!(!verify_result_1.program_result.is_err());

    // Second signature verification with the same verifier should fail
    let verify_result_2 = verify_signature_helper(
        &setup,
        payload_merkle_root,
        verifier_info,
        (
            verification_session_pda,
            verify_result_1
                .get_account(&verification_session_pda)
                .unwrap()
                .clone(),
        ),
        init_result
            .get_account(&setup.gateway_root_pda)
            .unwrap()
            .clone(),
        (
            setup.verifier_set_tracker_pda,
            init_result
                .get_account(&setup.verifier_set_tracker_pda)
                .unwrap()
                .clone(),
        ),
    );

    // Should fail with SlotAlreadyVerified error
    assert!(verify_result_2.program_result.is_err());
}

#[test]
#[allow(clippy::indexing_slicing)]
fn test_fails_when_approving_message_with_insufficient_signatures() {
    // Step 1: Setup gateway with real signers
    let (setup, secret_key_1, secret_key_2) = setup_test_with_real_signers();

    // Step 2: Initialize gateway
    let init_result = initialize_gateway(&setup);
    assert!(!init_result.program_result.is_err());

    // Step 3: Create messages and payload merkle root using std crate
    let messages = default_messages();
    let (merklized_messages, payload_merkle_root) =
        create_merklized_messages_from_std(setup.domain_separator, &messages);

    let payload_type = PayloadType::ApproveMessages;

    // Step 4: Initialize payload verification session
    let gateway_account = init_result
        .get_account(&setup.gateway_root_pda)
        .unwrap()
        .clone();
    let verifier_set_tracker_account = init_result
        .get_account(&setup.verifier_set_tracker_pda)
        .unwrap()
        .clone();

    let (session_result, verification_session_pda) = initialize_payload_verification_session(
        &setup,
        gateway_account,
        verifier_set_tracker_account,
        payload_merkle_root,
        payload_type,
    );
    assert!(!session_result.program_result.is_err());

    // Step 5: Get existing accounts
    let gateway_account = init_result
        .get_account(&setup.gateway_root_pda)
        .unwrap()
        .clone();

    let verifier_set_tracker_account = init_result
        .get_account(&setup.verifier_set_tracker_pda)
        .unwrap()
        .clone();

    let verification_session_account = session_result
        .get_account(&verification_session_pda)
        .unwrap()
        .clone();

    let payload_to_be_signed = Payload::Messages(Messages(messages));
    let signing_verifier_set_leaves = create_signing_verifier_set_leaves(
        setup.domain_separator,
        &secret_key_1,
        &secret_key_2,
        payload_to_be_signed,
        setup.verifier_set.clone(),
    );

    // Step 6: Sign the payload with ONLY ONE signer (not enough to make session valid)
    let verifier_info_1 = signing_verifier_set_leaves[0].clone();

    let verify_result_1 = verify_signature_helper(
        &setup,
        payload_merkle_root,
        verifier_info_1,
        (verification_session_pda, verification_session_account),
        gateway_account,
        (setup.verifier_set_tracker_pda, verifier_set_tracker_account),
    );
    assert!(!verify_result_1.program_result.is_err());

    // Step 7: Now try to approve a message with only one signature (insufficient)
    // The verification session should not be valid since we need both signers
    let gateway_account = verify_result_1
        .get_account(&setup.gateway_root_pda)
        .unwrap()
        .clone();
    let verification_session_account = verify_result_1
        .get_account(&verification_session_pda)
        .unwrap()
        .clone();

    let (approve_result, _) = approve_message_helper_from_merklized(
        &setup,
        &merklized_messages[0], // First message
        payload_merkle_root,
        (verification_session_pda, verification_session_account),
        gateway_account,
    );

    // Should fail because the verification session is not valid (insufficient signatures)
    assert!(
        approve_result.program_result.is_err(),
        "Approving message with insufficient signatures should fail, but got: {:?}",
        approve_result.program_result
    );
}

#[test]
#[allow(clippy::indexing_slicing)]
fn test_fails_when_verifying_invalid_signature() {
    // Step 1: Setup gateway with real signers
    let (setup, secret_key_1, secret_key_2) = setup_test_with_real_signers();

    // Step 2: Initialize gateway
    let init_result = initialize_gateway(&setup);
    assert!(!init_result.program_result.is_err());

    // Step 3: Create messages and payload merkle root
    let messages = default_messages();
    let (_, payload_merkle_root) =
        create_merklized_messages_from_std(setup.domain_separator, &messages);

    let payload_type = PayloadType::ApproveMessages;

    // Step 4: Initialize payload verification session with the correct payload root
    let gateway_account = init_result
        .get_account(&setup.gateway_root_pda)
        .unwrap()
        .clone();
    let verifier_set_tracker_account = init_result
        .get_account(&setup.verifier_set_tracker_pda)
        .unwrap()
        .clone();

    let (session_result, verification_session_pda) = initialize_payload_verification_session(
        &setup,
        gateway_account,
        verifier_set_tracker_account,
        payload_merkle_root,
        payload_type,
    );
    assert!(!session_result.program_result.is_err());

    // Step 5: Get existing accounts
    let gateway_account = init_result
        .get_account(&setup.gateway_root_pda)
        .unwrap()
        .clone();

    let verifier_set_tracker_account = init_result
        .get_account(&setup.verifier_set_tracker_pda)
        .unwrap()
        .clone();

    let verification_session_account = session_result
        .get_account(&verification_session_pda)
        .unwrap()
        .clone();

    let fake_messages = fake_messages();
    let payload_to_be_signed = Payload::Messages(Messages(fake_messages));
    let signing_verifier_set_leaves = create_signing_verifier_set_leaves(
        setup.domain_separator,
        &secret_key_1,
        &secret_key_2,
        payload_to_be_signed,
        setup.verifier_set.clone(),
    );

    // Step 6: Sign the payload with ONLY ONE signer (not enough to make session valid)
    let verifier_info_1 = signing_verifier_set_leaves[0].clone();

    // Step 7: Try to verify the invalid signature against the correct payload root
    let verify_result = verify_signature_helper(
        &setup,
        payload_merkle_root,
        verifier_info_1,
        (verification_session_pda, verification_session_account),
        gateway_account,
        (setup.verifier_set_tracker_pda, verifier_set_tracker_account),
    );

    assert!(
        verify_result.program_result.is_err(),
        "Verifying invalid signature should fail, but got: {:?}",
        verify_result.program_result
    );
}

#[test]
#[allow(clippy::indexing_slicing)]
fn test_fails_when_using_approve_messages_payload_for_rotate_signers() {
    // Step 1: Setup gateway with real signers
    let (setup, secret_key_1, secret_key_2) = setup_test_with_real_signers();

    // Step 2: Initialize gateway
    let init_result = initialize_gateway(&setup);
    assert!(!init_result.program_result.is_err());

    // Step 3: Create messages and payload merkle root for message approval
    let messages = default_messages();
    let (_, payload_merkle_root) =
        create_merklized_messages_from_std(setup.domain_separator, &messages);

    // Step 4: Initialize payload verification session with APPROVE MESSAGES command type
    let payload_type = PayloadType::ApproveMessages;
    let gateway_account = init_result
        .get_account(&setup.gateway_root_pda)
        .unwrap()
        .clone();
    let verifier_set_tracker_account = init_result
        .get_account(&setup.verifier_set_tracker_pda)
        .unwrap()
        .clone();

    let (session_result, verification_session_pda) = initialize_payload_verification_session(
        &setup,
        gateway_account,
        verifier_set_tracker_account,
        payload_merkle_root,
        payload_type,
    );
    assert!(!session_result.program_result.is_err());

    // Step 5: Get existing accounts
    let gateway_account = init_result
        .get_account(&setup.gateway_root_pda)
        .unwrap()
        .clone();

    let verifier_set_tracker_account = init_result
        .get_account(&setup.verifier_set_tracker_pda)
        .unwrap()
        .clone();

    let verification_session_account = session_result
        .get_account(&verification_session_pda)
        .unwrap()
        .clone();

    // Step 6: Sign the payload with both signers to complete the session
    let payload_to_be_signed = Payload::Messages(Messages(messages.clone()));
    let signing_verifier_set_leaves = create_signing_verifier_set_leaves(
        setup.domain_separator,
        &secret_key_1,
        &secret_key_2,
        payload_to_be_signed,
        setup.verifier_set.clone(),
    );

    let verifier_info_1 = signing_verifier_set_leaves[0].clone();

    // First signature verification should succeed
    let verify_result_1 = verify_signature_helper(
        &setup,
        payload_merkle_root,
        verifier_info_1,
        (
            verification_session_pda,
            verification_session_account.clone(),
        ),
        gateway_account.clone(),
        (
            setup.verifier_set_tracker_pda,
            verifier_set_tracker_account.clone(),
        ),
    );

    assert!(!verify_result_1.program_result.is_err());

    // Get updated verification session after first signature
    let updated_verification_account_after_first = verify_result_1
        .get_account(&verification_session_pda)
        .unwrap()
        .clone();

    // Second signer
    let verifier_info_2 = signing_verifier_set_leaves[1].clone();

    let verify_result_2 = verify_signature_helper(
        &setup,
        payload_merkle_root,
        verifier_info_2,
        (
            verification_session_pda,
            updated_verification_account_after_first,
        ),
        gateway_account,
        (setup.verifier_set_tracker_pda, verifier_set_tracker_account),
    );

    assert!(!verify_result_2.program_result.is_err());

    // Step 7: Verify the session is complete and valid
    let final_verification_account = verify_result_2
        .get_account(&verification_session_pda)
        .unwrap()
        .clone();

    let final_verification_session = SignatureVerificationSessionData::try_deserialize(
        &mut final_verification_account.data.as_slice(),
    )
    .unwrap();

    assert!(final_verification_session.signature_verification.is_valid());

    // try to use approve message payload_merkle_root for rotate_signers: should fail
    let final_gateway_account = verify_result_2
        .get_account(&setup.gateway_root_pda)
        .unwrap()
        .clone();
    let final_verification_session_account = verify_result_2
        .get_account(&verification_session_pda)
        .unwrap()
        .clone();
    let verifier_set_tracker_account = verify_result_2
        .get_account(&setup.verifier_set_tracker_pda)
        .unwrap()
        .clone();

    let rotate_result = rotate_signers_helper(
        &setup,
        payload_merkle_root, // Using the same payload_merkle_root as new_verifier_set_hash
        (verification_session_pda, final_verification_session_account),
        final_gateway_account,
        verifier_set_tracker_account,
    );

    assert!(rotate_result.program_result.is_err(),);
}
