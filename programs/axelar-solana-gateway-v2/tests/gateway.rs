use anchor_lang::prelude::ProgramError;
use anchor_lang::{AccountDeserialize, AnchorDeserialize};
use axelar_solana_encoding::hasher::NativeHasher;
use axelar_solana_encoding::types::execute_data::MerkleisedPayload;
use axelar_solana_encoding::types::messages::{Message, Messages};
use axelar_solana_encoding::types::payload::Payload;
use axelar_solana_encoding::types::verifier_set::verifier_set_hash;
use axelar_solana_gateway::instructions::approve_message;
use axelar_solana_gateway::instructions::validate_message;
use axelar_solana_gateway::state::incoming_message::command_id;
use axelar_solana_gateway::{
    get_gateway_root_config_pda, get_incoming_message_pda, get_validate_message_signing_pda,
    BytemuckedPda,
};
use axelar_solana_gateway_test_fixtures::gateway::{
    make_messages, make_verifier_set, random_bytes,
};
use axelar_solana_gateway_test_fixtures::SolanaAxelarIntegration;
use axelar_solana_gateway_v2::u256::U256;
use axelar_solana_gateway_v2::{
    signature_verification::{SignatureVerification, VerificationSessionAccount},
    state::VerifierSetTracker,
    GatewayConfig, ID as GATEWAY_PROGRAM_ID,
};
use axelar_solana_gateway_v2::{
    ApproveMessageInstruction, IncomingMessage, InitializePayloadVerificationSessionInstruction,
    MessageStatus, RotateSignersInstruction, ValidateMessageInstruction,
};
use axelar_solana_gateway_v2_test_fixtures::{
    approve_message_helper, call_contract_helper, create_verifier_info, initialize_gateway,
    initialize_payload_verification_session, initialize_payload_verification_session_with_root,
    mock_setup_test, rotate_signers_helper, setup_message_merkle_tree,
    setup_signer_rotation_payload, setup_test_with_real_signers, transfer_operatorship_helper,
    verify_signature_helper,
};
use solana_program::hash;
use solana_sdk::instruction::Instruction;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signer;

fn validate_message_for_tests(
    incoming_message_pda: &Pubkey,
    signing_pda: &Pubkey,
    message: Message,
) -> Result<Instruction, ProgramError> {
    let mut res = validate_message(incoming_message_pda, signing_pda, message)?;
    // needed because we cannot sign with a PDA without creating a real on-chain
    // program
    res.accounts[1].is_signer = false;
    Ok(res)
}

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

#[test]
fn test_call_contract_from_program() {
    let memo_program_id = Pubkey::new_unique();
    let setup = mock_setup_test(Some(memo_program_id));

    // Initialize gateway
    let init_result = initialize_gateway(&setup);
    assert!(
        !init_result.program_result.is_err(),
        "Gateway initialization should succeed"
    );

    let result = call_contract_helper(&setup, init_result, memo_program_id);

    assert!(
        !result.program_result.is_err(),
        "call_contract should succeed: {:?}",
        result.program_result
    );
}

#[test]
fn test_config_discriminator() {
    let gateway_caller = None;
    let setup = mock_setup_test(gateway_caller);
    let initialize_result = initialize_gateway(&setup);

    let updated_gateway_account = initialize_result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == setup.gateway_root_pda)
        .unwrap()
        .1
        .clone();

    let _ = GatewayConfig::try_deserialize(&mut updated_gateway_account.data.as_slice()).unwrap();

    let expected_discriminator = &hash::hash(b"account:GatewayConfig").to_bytes()[..8];
    println!("Discriminator: {:02x?}", expected_discriminator);
    let actual_discriminator = &updated_gateway_account.data.as_slice()[..8];
    assert_eq!(actual_discriminator, expected_discriminator);

    let gateway_config =
        axelar_solana_gateway::state::GatewayConfig::read(&updated_gateway_account.data).unwrap();

    assert_eq!(gateway_config.operator, setup.operator);
    assert_eq!(gateway_config.minimum_rotation_delay, 3600);
    assert_eq!(gateway_config.domain_separator, [2u8; 32]);
}

#[tokio::test]
async fn test_initialize_config_discriminator() {
    // Create V1 instruction
    let metadata = SolanaAxelarIntegration::builder()
        .initial_signer_weights(vec![42])
        .build()
        .setup_without_init_config()
        .await;
    let (gateway_config_pda, _bump) = get_gateway_root_config_pda();
    let initial_sets = metadata.init_gateway_config_verifier_set_data();

    let v1_ix = axelar_solana_gateway::instructions::initialize_config(
        metadata.fixture.payer.pubkey(),
        metadata.upgrade_authority.pubkey(),
        metadata.domain_separator,
        initial_sets.clone(),
        metadata.minimum_rotate_signers_delay_seconds,
        metadata.operator.pubkey(),
        metadata.previous_signers_retention.into(),
        gateway_config_pda,
    )
    .unwrap();

    // Check the V1 discriminator
    let v1_discriminator: [u8; 8] = v1_ix.data[..8].to_vec().try_into().unwrap();

    // Check V2's expected discriminator
    let v2_discriminator: [u8; 8] = hash::hash(b"global:initialize_config").to_bytes()[..8]
        .to_vec()
        .try_into()
        .unwrap();

    assert_eq!(
        v1_discriminator, v2_discriminator,
        "Discriminators should match for backwards compatibility"
    );

    let v2_parsed =
        axelar_solana_gateway_v2::state::config::InitializeConfig::try_from_slice(&v1_ix.data[8..]);
    let parsed_config = v2_parsed.expect("Failed to parse V1 instruction as V2 InitializeConfig");

    assert_eq!(parsed_config.domain_separator, [42; 32]);
    assert_eq!(
        parsed_config.operator.to_string(),
        metadata.operator.pubkey().to_string()
    );
    assert_eq!(parsed_config.previous_verifier_retention, U256::from(1));
    assert_eq!(parsed_config.minimum_rotation_delay, 0);
    assert_eq!(parsed_config.initial_verifier_set.hash, initial_sets.hash);
}

#[tokio::test]
async fn test_initialize_payload_verification_session_discriminator() {
    // Setup
    let metadata = SolanaAxelarIntegration::builder()
        .initial_signer_weights(vec![42])
        .build()
        .setup()
        .await;

    // Action
    let payload_merkle_root = random_bytes();
    let gateway_config_pda = get_gateway_root_config_pda().0;

    let v1_ix = axelar_solana_gateway::instructions::initialize_payload_verification_session(
        metadata.payer.pubkey(),
        gateway_config_pda,
        payload_merkle_root,
    )
    .unwrap();

    let v1_discriminator: [u8; 8] = v1_ix.data[..8].to_vec().try_into().unwrap();
    let v2_discriminator: [u8; 8] = hash::hash(b"global:initialize_payload_verification_session")
        .to_bytes()[..8]
        .to_vec()
        .try_into()
        .unwrap();

    assert_eq!(
        v1_discriminator, v2_discriminator,
        "Discriminators should match for backwards compatibility"
    );

    let v2_parsed =
        InitializePayloadVerificationSessionInstruction::try_from_slice(&v1_ix.data[8..]).unwrap();
    assert_eq!(v2_parsed.payload_merkle_root, payload_merkle_root);
}

#[tokio::test]
async fn test_approve_message_discriminator() {
    // Setup
    let mut metadata = SolanaAxelarIntegration::builder()
        .initial_signer_weights(vec![42, 42])
        .build()
        .setup()
        .await;
    let message_count = 1;
    let messages = make_messages(message_count);
    let payload = Payload::Messages(Messages(messages.clone()));
    let execute_data = metadata.construct_execute_data(&metadata.signers.clone(), payload);
    let verification_session_pda = metadata
        .init_payload_session_and_verify(&execute_data)
        .await
        .unwrap();

    let MerkleisedPayload::NewMessages { messages } = execute_data.payload_items else {
        unreachable!()
    };

    let message_info = messages.get(0).unwrap();
    let command_id = command_id(
        &message_info.leaf.message.cc_id.chain,
        &message_info.leaf.message.cc_id.id,
    );
    let (incoming_message_pda, _) = get_incoming_message_pda(&command_id);

    let message = message_info.leaf.clone().message;
    let v1_ix = approve_message(
        message_info.clone(),
        execute_data.payload_merkle_root,
        metadata.gateway_root_pda,
        metadata.payer.pubkey(),
        verification_session_pda,
        incoming_message_pda,
    )
    .unwrap();

    let v1_discriminator: [u8; 8] = v1_ix.data[..8].to_vec().try_into().unwrap();
    let v2_discriminator: [u8; 8] = hash::hash(b"global:approve_message").to_bytes()[..8]
        .to_vec()
        .try_into()
        .unwrap();

    assert_eq!(
        v1_discriminator, v2_discriminator,
        "Discriminators should match for backwards compatibility"
    );

    let v2_parsed = ApproveMessageInstruction::try_from_slice(&v1_ix.data[8..]).unwrap();

    assert_eq!(
        v2_parsed.message.leaf.message.cc_id.chain,
        message.cc_id.chain
    );
    assert_eq!(v2_parsed.message.leaf.message.cc_id.id, message.cc_id.id);
    assert_eq!(
        v2_parsed.message.leaf.message.source_address,
        message.source_address
    );
    assert_eq!(
        v2_parsed.message.leaf.message.destination_chain,
        message.destination_chain
    );
    assert_eq!(
        v2_parsed.message.leaf.message.destination_address,
        message.destination_address
    );
    assert_eq!(
        v2_parsed.message.leaf.message.payload_hash,
        message.payload_hash
    );
}

#[tokio::test]
async fn test_rotate_signers_discriminator() {
    // Setup
    let mut metadata = SolanaAxelarIntegration::builder()
        .initial_signer_weights(vec![42, 42])
        .build()
        .setup()
        .await;
    let new_verifier_set = make_verifier_set(&[500, 200], 1, metadata.domain_separator);
    let payload = Payload::NewVerifierSet(new_verifier_set.verifier_set());
    let execute_data = metadata.construct_execute_data(&metadata.signers.clone(), payload);
    let new_verifier_set_hash = verifier_set_hash::<NativeHasher>(
        &new_verifier_set.verifier_set(),
        &metadata.domain_separator,
    )
    .unwrap();
    let MerkleisedPayload::VerifierSetRotation {
        new_verifier_set_merkle_root,
    } = execute_data.payload_items
    else {
        unreachable!()
    };
    let verification_session_account = metadata
        .init_payload_session_and_verify(&execute_data)
        .await
        .unwrap();

    let v1_ix = axelar_solana_gateway::instructions::rotate_signers(
        metadata.gateway_root_pda,
        verification_session_account,
        metadata.signers.verifier_set_tracker().0,
        axelar_solana_gateway::get_verifier_set_tracker_pda(new_verifier_set_merkle_root).0,
        metadata.payer.pubkey(),
        None,
        new_verifier_set_hash,
    )
    .unwrap();

    let v1_discriminator: [u8; 8] = v1_ix.data[..8].to_vec().try_into().unwrap();
    let v2_discriminator: [u8; 8] = hash::hash(b"global:rotate_signers").to_bytes()[..8]
        .to_vec()
        .try_into()
        .unwrap();

    assert_eq!(
        v1_discriminator, v2_discriminator,
        "Discriminators should match for backwards compatibility"
    );

    let v2_parsed = RotateSignersInstruction::try_from_slice(&v1_ix.data[8..]).unwrap();
    assert_eq!(
        v2_parsed.new_verifier_set_merkle_root,
        new_verifier_set_hash
    );
}

#[tokio::test]
async fn test_validate_message_discriminator() {
    // Setup
    let mut metadata = SolanaAxelarIntegration::builder()
        .initial_signer_weights(vec![42, 42])
        .build()
        .setup()
        .await;
    let mut messages = make_messages(1);
    let destination_address = Pubkey::new_unique();
    if let Some(x) = messages.get_mut(0) {
        x.destination_address = destination_address.to_string();
    }
    let message_leaf = metadata
        .sign_session_and_approve_messages(&metadata.signers.clone(), &messages)
        .await
        .unwrap()
        .into_iter()
        .next()
        .unwrap()
        .leaf;
    let fake_command_id = solana_program::keccak::hash(b"fake command id").0; // source of error -- invalid command id
    let (incoming_message_pda, ..) = get_incoming_message_pda(&fake_command_id);

    // action
    let (signing_pda, _signing_pda_bump) =
        get_validate_message_signing_pda(destination_address, fake_command_id);
    let v1_ix =
        validate_message_for_tests(&incoming_message_pda, &signing_pda, message_leaf.message)
            .unwrap();

    let v1_discriminator: [u8; 8] = v1_ix.data[..8].to_vec().try_into().unwrap();
    let v2_discriminator: [u8; 8] = hash::hash(b"global:validate_message").to_bytes()[..8]
        .to_vec()
        .try_into()
        .unwrap();

    assert_eq!(
        v1_discriminator, v2_discriminator,
        "Discriminators should match for backwards compatibility"
    );

    let v2_parsed = ValidateMessageInstruction::try_from_slice(&v1_ix.data[8..]).unwrap();
    let message = &messages[0];
    assert_eq!(v2_parsed.message.cc_id.chain, message.cc_id.chain);
    assert_eq!(v2_parsed.message.cc_id.id, message.cc_id.id);
    assert_eq!(v2_parsed.message.source_address, message.source_address);
    assert_eq!(
        v2_parsed.message.destination_chain,
        message.destination_chain
    );
    assert_eq!(
        v2_parsed.message.destination_address,
        message.destination_address
    );
    assert_eq!(v2_parsed.message.payload_hash, message.payload_hash);
}
