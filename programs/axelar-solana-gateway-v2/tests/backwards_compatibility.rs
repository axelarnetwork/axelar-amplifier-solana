use anchor_lang::prelude::ProgramError;
use anchor_lang::{AccountDeserialize, AnchorDeserialize};
use axelar_solana_encoding::hasher::NativeHasher;
use axelar_solana_encoding::types::execute_data::MerkleisedPayload;
use axelar_solana_encoding::types::messages::{Message, Messages};
use axelar_solana_encoding::types::payload::Payload;
use axelar_solana_encoding::types::verifier_set::verifier_set_hash;
use axelar_solana_gateway::instructions::{approve_message, validate_message};
use axelar_solana_gateway::state::incoming_message::command_id;
use axelar_solana_gateway::{
    get_gateway_root_config_pda, get_incoming_message_pda, get_validate_message_signing_pda,
    BytemuckedPda,
};
use axelar_solana_gateway_test_fixtures::gateway::{
    make_messages, make_verifier_set, random_bytes, random_message,
};
use axelar_solana_gateway_test_fixtures::SolanaAxelarIntegration;
use axelar_solana_gateway_v2::u256::U256;
use axelar_solana_gateway_v2::{
    ApproveMessageInstruction, GatewayConfig, InitializeConfigInstruction,
    InitializePayloadVerificationSessionInstruction, RotateSignersInstruction,
    ValidateMessageInstruction, VerifySignatureInstruction,
};
use axelar_solana_gateway_v2_test_fixtures::{initialize_gateway, mock_setup_test};
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
    res.accounts[1].is_signer = false;
    Ok(res)
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

    let v2_parsed = InitializeConfigInstruction::try_from_slice(&v1_ix.data[8..]);
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

#[tokio::test]
async fn test_verify_signature_discriminator() {
    // Define test cases
    let test_cases = vec![(vec![42], Messages(vec![random_message()]))];

    for (initial_signer_weights, messages) in test_cases {
        // Setup
        let mut metadata = SolanaAxelarIntegration::builder()
            .initial_signer_weights(initial_signer_weights.clone())
            .build()
            .setup()
            .await;

        let payload = Payload::Messages(messages);
        let execute_data = metadata.construct_execute_data(&metadata.signers.clone(), payload);
        metadata
            .initialize_payload_verification_session(&execute_data)
            .await
            .unwrap();
        let verifier_set_tracker_pda = metadata.signers.verifier_set_tracker().0;
        let leaf_info = execute_data.signing_verifier_set_leaves.first().unwrap();

        // Verify the signature
        let v1_ix = axelar_solana_gateway::instructions::verify_signature(
            metadata.gateway_root_pda,
            verifier_set_tracker_pda,
            execute_data.payload_merkle_root,
            leaf_info.clone(),
        )
        .unwrap();

        let v1_discriminator: [u8; 8] = v1_ix.data[..8].to_vec().try_into().unwrap();
        let v2_discriminator: [u8; 8] = hash::hash(b"global:verify_signature").to_bytes()[..8]
            .to_vec()
            .try_into()
            .unwrap();

        assert_eq!(
            v1_discriminator, v2_discriminator,
            "Discriminators should match for backwards compatibility"
        );

        let v2_parsed = VerifySignatureInstruction::try_from_slice(&v1_ix.data[8..]).unwrap();
        assert_eq!(
            v2_parsed.payload_merkle_root,
            execute_data.payload_merkle_root
        );

        let expected_leaf = &leaf_info.leaf;
        assert_eq!(v2_parsed.verifier_info.leaf.nonce, expected_leaf.nonce);
        assert_eq!(v2_parsed.verifier_info.leaf.quorum, expected_leaf.quorum);
        assert_eq!(
            v2_parsed.verifier_info.leaf.signer_weight,
            expected_leaf.signer_weight
        );
        assert_eq!(
            v2_parsed.verifier_info.leaf.position,
            expected_leaf.position
        );
        assert_eq!(
            v2_parsed.verifier_info.leaf.set_size,
            expected_leaf.set_size
        );
        assert_eq!(
            v2_parsed.verifier_info.leaf.domain_separator,
            expected_leaf.domain_separator
        );
        assert_eq!(v2_parsed.verifier_info.merkle_proof, leaf_info.merkle_proof);
    }
}

#[test]
fn test_message_leaf_hash_compatibility() {
    use axelar_solana_encoding::hasher::SolanaSyscallHasher;
    use axelar_solana_encoding::types::messages::CrossChainId as V1CrossChainId;
    use axelar_solana_encoding::types::messages::Message as V1Message;
    use axelar_solana_encoding::types::messages::MessageLeaf as V1MessageLeaf;
    use axelar_solana_encoding::LeafHash;
    use axelar_solana_gateway_v2::state::message_approval::{
        CrossChainId as V2CrossChainId, Message as V2Message, MessageLeaf as V2MessageLeaf,
    };

    // Create test data
    let chain = "ethereum".to_string();
    let id = "0x1234567890abcdef".to_string();
    let source_address = "0xabcdef1234567890".to_string();
    let destination_chain = "solana".to_string();
    let destination_address = "11111111111111111111111111111112".to_string();
    let payload_hash = [1u8; 32];
    let position = 0u16;
    let set_size = 1u16;
    let domain_separator = [42u8; 32];
    let signing_verifier_set = [84u8; 32];

    // Create V1 message and leaf
    let v1_cc_id = V1CrossChainId {
        chain: chain.clone(),
        id: id.clone(),
    };
    let v1_message = V1Message {
        cc_id: v1_cc_id,
        source_address: source_address.clone(),
        destination_chain: destination_chain.clone(),
        destination_address: destination_address.clone(),
        payload_hash,
    };
    let v1_leaf = V1MessageLeaf {
        message: v1_message,
        position,
        set_size,
        domain_separator,
        signing_verifier_set,
    };

    // Create V2 message and leaf
    let v2_cc_id = V2CrossChainId {
        chain: chain.clone(),
        id: id.clone(),
    };
    let v2_message = V2Message {
        cc_id: v2_cc_id,
        source_address: source_address.clone(),
        destination_chain: destination_chain.clone(),
        destination_address: destination_address.clone(),
        payload_hash,
    };
    let v2_leaf = V2MessageLeaf {
        message: v2_message,
        position,
        set_size,
        domain_separator,
        signing_verifier_set,
    };

    // Hash with V1 (using LeafHash trait with SolanaSyscallHasher)
    let v1_hash = v1_leaf.hash::<SolanaSyscallHasher>();

    // Hash with V2 (using the current borsh implementation)
    let v2_hash = v2_leaf.hash();

    // They should match for backwards compatibility
    assert_eq!(
        v1_hash, v2_hash,
        "Message leaf hashes should match between v1 and v2 implementations.\nV1 hash: {:?}\nV2 hash: {:?}",
        v1_hash, v2_hash
    );
}
