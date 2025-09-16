use anchor_lang::prelude::ProgramError;
use axelar_solana_encoding::types::messages::Message;
use axelar_solana_gateway::instructions::validate_message;
use solana_program::hash;
use solana_sdk::{instruction::Instruction, pubkey::Pubkey};

fn validate_message_for_tests(
    incoming_message_pda: &Pubkey,
    signing_pda: &Pubkey,
    message: Message,
) -> Result<Instruction, ProgramError> {
    let mut res = validate_message(incoming_message_pda, signing_pda, message)?;
    res.accounts[1].is_signer = false;
    Ok(res)
}

fn discriminator(ix: &Instruction) -> [u8; 8] {
    ix.data[..8].try_into().unwrap()
}

fn expected_discriminator(name: &str) -> [u8; 8] {
    hash::hash(format!("global:{name}").as_bytes()).to_bytes()[..8]
        .try_into()
        .unwrap()
}

mod pda_compatibility {
    use anchor_lang::AccountDeserialize;
    use axelar_solana_gateway::BytemuckedPda;
    use axelar_solana_gateway_v2::{
        GatewayConfig, IncomingMessage, SignatureVerificationSessionData, VerifierSetTracker,
    };
    use axelar_solana_gateway_v2_test_fixtures::{
        approve_message_helper, create_verifier_info, initialize_gateway,
        initialize_payload_verification_session_with_root, mock_setup_test,
        setup_message_merkle_tree, setup_test_with_real_signers, verify_signature_helper,
    };
    use solana_program::hash;

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

        let _ =
            GatewayConfig::try_deserialize(&mut updated_gateway_account.data.as_slice()).unwrap();

        let expected_discriminator = &hash::hash(b"account:GatewayConfig").to_bytes()[..8];
        let actual_discriminator = &updated_gateway_account.data.as_slice()[..8];
        assert_eq!(actual_discriminator, expected_discriminator);

        let gateway_config =
            axelar_solana_gateway::state::GatewayConfig::read(&updated_gateway_account.data)
                .unwrap();

        assert_eq!(gateway_config.discriminator, actual_discriminator);
        assert_eq!(gateway_config.operator, setup.operator);
        assert_eq!(gateway_config.minimum_rotation_delay, 3600);
        assert_eq!(gateway_config.domain_separator, [2u8; 32]);
    }

    #[test]
    fn test_verifier_set_tracker_discriminator() {
        let gateway_caller = None;
        let setup = mock_setup_test(gateway_caller);
        let initialize_result = initialize_gateway(&setup);

        let verifier_set_tracker_account = initialize_result
            .resulting_accounts
            .iter()
            .find(|(pubkey, _)| *pubkey == setup.verifier_set_tracker_pda)
            .unwrap()
            .1
            .clone();

        let verifier_set_tracker_v2 =
            VerifierSetTracker::try_deserialize(&mut verifier_set_tracker_account.data.as_slice())
                .unwrap();

        let expected_discriminator = &hash::hash(b"account:VerifierSetTracker").to_bytes()[..8];
        let actual_discriminator = &verifier_set_tracker_account.data.as_slice()[..8];
        assert_eq!(actual_discriminator, expected_discriminator);

        let verifier_set_tracker_v1 =
            axelar_solana_gateway::state::verifier_set_tracker::VerifierSetTracker::read(
                &verifier_set_tracker_account.data,
            )
            .unwrap();

        let v1_discriminator = verifier_set_tracker_v1.discriminator;
        assert_eq!(actual_discriminator, v1_discriminator);

        assert_eq!(verifier_set_tracker_v1.bump, verifier_set_tracker_v2.bump);

        assert_eq!(
            verifier_set_tracker_v1.epoch.to_le_bytes(),
            verifier_set_tracker_v2.epoch.to_le_bytes(),
        );

        let expected_hash = [1u8; 32];
        assert_eq!(verifier_set_tracker_v2.verifier_set_hash, expected_hash);

        let v1_hash_bytes: [u8; 32] = verifier_set_tracker_v1
            .verifier_set_hash
            .try_into()
            .unwrap();
        assert_eq!(v1_hash_bytes, expected_hash);
    }

    #[test]
    fn test_verification_session_tracker_discriminator() {
        // Step 1: Setup gateway with real signers
        let (setup, _, _, _, _) = setup_test_with_real_signers();

        // Step 2: Initialize gateway
        let init_result = initialize_gateway(&setup);

        // Step 3: Create messages and payload merkle root
        let verifier_set_merkle_root = setup.verifier_set_hash;
        let (_, _, _, payload_merkle_root) =
            setup_message_merkle_tree(&setup, verifier_set_merkle_root);

        // Step 4: Initialize payload verification session
        let (session_result, verification_session_pda) =
            initialize_payload_verification_session_with_root(
                &setup,
                &init_result,
                payload_merkle_root,
            );

        let verification_session_account = session_result
            .resulting_accounts
            .iter()
            .find(|(pubkey, _)| *pubkey == verification_session_pda)
            .unwrap()
            .1
            .clone();

        let verification_session_account_v2 = SignatureVerificationSessionData::try_deserialize(
            &mut verification_session_account.data.as_slice(),
        )
        .unwrap();

        let expected_discriminator =
            &hash::hash(b"account:SignatureVerificationSessionData").to_bytes()[..8];
        let actual_discriminator = &verification_session_account.data.as_slice()[..8];
        assert_eq!(actual_discriminator, expected_discriminator);

        let verification_session_account_v1 =
            axelar_solana_gateway::state::signature_verification_pda::SignatureVerificationSessionData::read(
                &verification_session_account.data,
            )
            .unwrap();

        assert_eq!(
            verification_session_account_v1.discriminator,
            actual_discriminator
        );

        assert_eq!(
            verification_session_account_v1.bump,
            verification_session_account_v2.bump
        );

        let sig_v1 = &verification_session_account_v1.signature_verification;
        let sig_v2 = &verification_session_account_v2.signature_verification;

        assert_eq!(sig_v1.accumulated_threshold, sig_v2.accumulated_threshold);
        assert_eq!(sig_v1.signature_slots, sig_v2.signature_slots);
        assert_eq!(
            sig_v1.signing_verifier_set_hash,
            sig_v2.signing_verifier_set_hash
        );
    }

    #[test]
    fn test_incoming_message_discriminator() {
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

        let incoming_message_account = approve_result
            .resulting_accounts
            .iter()
            .find(|(pubkey, _)| *pubkey == incoming_message_pda)
            .unwrap()
            .1
            .clone();

        let incoming_message_pda_account_v2 =
            IncomingMessage::try_deserialize(&mut incoming_message_account.data.as_slice())
                .unwrap();

        let expected_discriminator = &hash::hash(b"account:IncomingMessage").to_bytes()[..8];
        let actual_discriminator = &incoming_message_account.data.as_slice()[..8];
        assert_eq!(actual_discriminator, expected_discriminator);

        let incoming_message_pda_account_v1 =
            axelar_solana_gateway::state::incoming_message::IncomingMessage::read(
                &incoming_message_account.data,
            )
            .unwrap();

        assert_eq!(
            incoming_message_pda_account_v1.discriminator,
            actual_discriminator
        );

        assert_eq!(
            incoming_message_pda_account_v1.bump,
            incoming_message_pda_account_v2.bump
        );
        assert_eq!(
            incoming_message_pda_account_v1.signing_pda_bump,
            incoming_message_pda_account_v2.signing_pda_bump
        );

        assert_eq!(
            incoming_message_pda_account_v1.status.is_approved(),
            incoming_message_pda_account_v2.status.is_approved()
        );
        assert_eq!(
            incoming_message_pda_account_v1.message_hash,
            incoming_message_pda_account_v2.message_hash
        );
        assert_eq!(
            incoming_message_pda_account_v1.payload_hash,
            incoming_message_pda_account_v2.payload_hash
        );
    }
}

mod instruction_compatibility {
    use crate::{discriminator, expected_discriminator, validate_message_for_tests};
    use anchor_lang::AnchorDeserialize;
    use axelar_solana_encoding::hasher::NativeHasher;
    use axelar_solana_encoding::types::execute_data::MerkleisedPayload;
    use axelar_solana_encoding::types::messages::Messages;
    use axelar_solana_encoding::types::payload::Payload;
    use axelar_solana_encoding::types::verifier_set::verifier_set_hash;
    use axelar_solana_gateway::instructions::approve_message;
    use axelar_solana_gateway::state::incoming_message::command_id;
    use axelar_solana_gateway::{
        get_gateway_root_config_pda, get_incoming_message_pda, get_validate_message_signing_pda,
    };
    use axelar_solana_gateway_test_fixtures::gateway::{
        make_messages, make_verifier_set, random_bytes, random_message,
    };
    use axelar_solana_gateway_test_fixtures::SolanaAxelarIntegration;
    use axelar_solana_gateway_v2::u256::U256;
    use axelar_solana_gateway_v2::{
        ApproveMessageInstruction, InitializeConfigInstruction,
        InitializePayloadVerificationSessionInstruction, RotateSignersInstruction,
        ValidateMessageInstruction, VerifySignatureInstruction,
    };
    use solana_sdk::pubkey::Pubkey;
    use solana_sdk::signature::Signer;

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

        assert_eq!(
            discriminator(&v1_ix),
            expected_discriminator("initialize_config"),
            "Discriminators should match for backwards compatibility"
        );

        let v2_parsed = InitializeConfigInstruction::try_from_slice(&v1_ix.data[8..]);
        let parsed_config = v2_parsed.unwrap();

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

        assert_eq!(
            discriminator(&v1_ix),
            expected_discriminator("initialize_payload_verification_session"),
            "Discriminators should match for backwards compatibility"
        );

        let v2_parsed =
            InitializePayloadVerificationSessionInstruction::try_from_slice(&v1_ix.data[8..])
                .unwrap();
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

        assert_eq!(
            discriminator(&v1_ix),
            expected_discriminator("approve_message"),
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

        assert_eq!(
            discriminator(&v1_ix),
            expected_discriminator("rotate_signers"),
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

        assert_eq!(
            discriminator(&v1_ix),
            expected_discriminator("validate_message"),
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

            assert_eq!(
                discriminator(&v1_ix),
                expected_discriminator("verify_signature"),
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
}

mod hash_compatibility {
    use axelar_solana_encoding::hasher::SolanaSyscallHasher;
    use axelar_solana_encoding::types::messages::CrossChainId as V1CrossChainId;
    use axelar_solana_encoding::types::messages::Message as V1Message;
    use axelar_solana_encoding::types::messages::MessageLeaf as V1MessageLeaf;
    use axelar_solana_encoding::types::pubkey::PublicKey as V1PublicKey;
    use axelar_solana_encoding::types::verifier_set::VerifierSetLeaf as V1VerifierSetLeaf;
    use axelar_solana_encoding::LeafHash;
    use axelar_solana_gateway_v2::state::message_approval::MessageLeaf as V2MessageLeaf;
    use axelar_solana_gateway_v2::state::message_approval::{
        CrossChainId as V2CrossChainId, Message as V2Message,
    };
    use axelar_solana_gateway_v2::state::signature_verification::VerifierSetLeaf as V2VerifierSetLeaf;
    use axelar_solana_gateway_v2::types::pubkey::PublicKey as V2PublicKey;

    #[test]
    fn test_message_leaf_hash_compatibility() {
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

        let v1_hash = v1_leaf.hash::<SolanaSyscallHasher>();
        let v2_hash = v2_leaf.hash();

        assert_eq!(
            v1_hash, v2_hash,
            "Message leaf hashes should match between v1 and v2 implementations.\nV1 hash: {:?}\nV2 hash: {:?}",
            v1_hash, v2_hash
        );
    }

    #[test]
    fn test_message_hash_compatibility() {
        let chain = "ethereum".to_string();
        let id = "0x1234567890abcdef".to_string();
        let source_address = "0xabcdef1234567890".to_string();
        let destination_chain = "solana".to_string();
        let destination_address = "11111111111111111111111111111112".to_string();
        let payload_hash = [1u8; 32];

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

        let v1_hash = v1_message.hash::<SolanaSyscallHasher>();
        let v2_hash = v2_message.hash();

        assert_eq!(
            v1_hash, v2_hash,
            "Message leaf hashes should match between v1 and v2 implementations.\nV1 hash: {:?}\nV2 hash: {:?}",
            v1_hash, v2_hash
        );
    }

    #[test]
    fn test_verifier_set_leaf_hash_compatibility() {
        let nonce = 12345u64;
        let quorum = 666u128;
        let signer_weight = 100u128;
        let position = 5u16;
        let set_size = 10u16;
        let domain_separator = [42u8; 32];

        let secp256k1_pubkey = [
            0x02, 0x79, 0xbe, 0x66, 0x7e, 0xf9, 0xdc, 0xbb, 0xac, 0x55, 0xa0, 0x62, 0x95, 0xce,
            0x87, 0x0b, 0x07, 0x02, 0x9b, 0xfc, 0xdb, 0x2d, 0xce, 0x28, 0xd9, 0x59, 0xf2, 0x81,
            0x5b, 0x16, 0xf8, 0x17, 0x98,
        ];

        let v1_signer_pubkey = V1PublicKey::Secp256k1(secp256k1_pubkey);
        let v1_leaf = V1VerifierSetLeaf {
            nonce,
            quorum,
            signer_pubkey: v1_signer_pubkey,
            signer_weight,
            position,
            set_size,
            domain_separator,
        };

        let v2_signer_pubkey = V2PublicKey::Secp256k1(secp256k1_pubkey);
        let v2_leaf = V2VerifierSetLeaf {
            nonce,
            quorum,
            signer_pubkey: v2_signer_pubkey,
            signer_weight,
            position,
            set_size,
            domain_separator,
        };

        let v1_hash = v1_leaf.hash::<SolanaSyscallHasher>();
        let v2_hash = v2_leaf.hash();

        assert_eq!(
            v1_hash, v2_hash,
            "VerifierSetLeaf hashes should match between v1 and v2 implementations.\nV1 hash: {:?}\nV2 hash: {:?}",
            v1_hash, v2_hash
        );
    }
}
