#![cfg(test)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::indexing_slicing)]

use anchor_lang::AccountDeserialize;
use anchor_spl::associated_token::get_associated_token_address_with_program_id;
use anchor_spl::token_2022::spl_token_2022;
use interchain_token_transfer_gmp::{GMPPayload, LinkToken, ReceiveFromHub};
use mollusk_svm::result::Check;
use solana_axelar_gateway::{GatewayConfig, ID as GATEWAY_PROGRAM_ID};
use solana_axelar_gateway_test_fixtures::{
    approve_messages_on_gateway, create_test_message, initialize_gateway,
    setup_test_with_real_signers,
};
use solana_axelar_its::ItsError;
use solana_axelar_its::{state::TokenManager, utils::interchain_token_id};
use solana_axelar_its_test_fixtures::{
    create_test_mint, execute_its_instruction, init_its_service_with_ethereum_trusted,
    initialize_mollusk, link_token_extra_accounts, new_empty_account, new_test_account,
    ExecuteTestAccounts, ExecuteTestContext, ExecuteTestParams,
};
use solana_sdk::{keccak, pubkey::Pubkey};

#[test]
fn test_execute_link_token() {
    // Step 1: Setup gateway with real signers
    let (mut setup, verifier_leaves, verifier_merkle_tree, secret_key_1, secret_key_2) =
        setup_test_with_real_signers();

    // Step 2: Initialize gateway
    let init_result = initialize_gateway(&setup);
    let (gateway_root_pda, _) = GatewayConfig::find_pda();
    let gateway_root_pda_account = init_result.get_account(&gateway_root_pda).unwrap();

    assert!(init_result.program_result.is_ok());

    // Step 3: Add ITS program to mollusk - USE the properly configured mollusk
    let program_id = solana_axelar_its::id();

    // Use the properly configured mollusk that has Token2022 and other programs
    let mut mollusk = initialize_mollusk();

    // We still need to add the gateway program since initialize_mollusk doesn't include it for execution tests
    mollusk.add_program(
        &GATEWAY_PROGRAM_ID,
        "../../target/deploy/solana_axelar_gateway",
        &solana_sdk::bpf_loader_upgradeable::id(),
    );

    // Update setup to use our properly configured mollusk
    setup.mollusk = mollusk;

    // Step 4: Initialize ITS service
    let (payer, payer_account) = new_test_account();
    let (operator, operator_account) = new_test_account();

    let chain_name = "solana".to_owned();
    let its_hub_address = "0x123456789abcdef".to_owned();

    // Use our configured mollusk for ITS service initialization too
    let (its_root_pda, its_root_account) = init_its_service_with_ethereum_trusted(
        &setup.mollusk, // Now this mollusk has Token2022 properly configured
        payer,
        &payer_account,
        payer,
        operator,
        &operator_account,
        chain_name.clone(),
        its_hub_address.clone(),
    );

    // Step 5: Create existing token mint (key difference from deploy test)
    let mint_authority = payer; // Use payer as mint authority for simplicity
    let (existing_token_mint, existing_token_mint_account) = create_test_mint(mint_authority);

    // Step 6: Create token linking parameters
    let salt = [1u8; 32];
    let token_id = interchain_token_id(&payer, &salt);
    let token_manager_type = 1u8; // LockUnlock type
    let source_token_address = existing_token_mint.to_bytes().to_vec();
    let destination_token_address = existing_token_mint.to_bytes().to_vec();
    let link_params = vec![]; // No additional params (no operator)

    // Step 7: Create the GMP payload
    let link_payload = LinkToken {
        selector: alloy_primitives::U256::from(5), // MESSAGE_TYPE_ID for LinkToken
        token_id: alloy_primitives::FixedBytes::from(token_id),
        token_manager_type: alloy_primitives::U256::from(token_manager_type),
        source_token_address: alloy_primitives::Bytes::from(source_token_address),
        destination_token_address: alloy_primitives::Bytes::from(destination_token_address),
        link_params: alloy_primitives::Bytes::from(link_params.clone()),
    };

    // Wrap in ReceiveFromHub payload
    let receive_from_hub_payload = ReceiveFromHub {
        selector: alloy_primitives::U256::from(4), // MESSAGE_TYPE_ID for ReceiveFromHub
        source_chain: "ethereum".to_owned(),
        payload: GMPPayload::LinkToken(link_payload).encode().into(),
    };

    let gmp_payload = GMPPayload::ReceiveFromHub(receive_from_hub_payload);
    let encoded_payload = gmp_payload.encode();
    let payload_hash = keccak::hashv(&[&encoded_payload]).to_bytes();

    // Step 8: Create test message
    let mut message = create_test_message(
        "ethereum",
        "link_token_123",
        &program_id.to_string(),
        payload_hash,
    );

    // Override the source_address to match its_hub_address
    message.source_address = its_hub_address.clone();

    // Step 9: Approve message on gateway
    let incoming_messages = approve_messages_on_gateway(
        &setup,
        vec![message.clone()],
        init_result.clone(),
        &secret_key_1,
        &secret_key_2,
        verifier_leaves,
        verifier_merkle_tree,
    );

    let (_, incoming_message_pda, incoming_message_account_data) = &incoming_messages[0];
    let (token_manager_pda, _) = TokenManager::find_pda(token_id, its_root_pda);

    // For link token, we use the existing mint, not a new PDA
    let token_mint_pda = existing_token_mint;

    let token_manager_ata = get_associated_token_address_with_program_id(
        &token_manager_pda,
        &token_mint_pda,
        &spl_token_2022::id(),
    );

    let context = ExecuteTestContext {
        mollusk: setup.mollusk,
        gateway_root: (gateway_root_pda, gateway_root_pda_account.clone()),
        its_root: (its_root_pda, its_root_account),
        payer: (payer, payer_account.clone()),
    };

    let params = ExecuteTestParams {
        message: message.clone(),
        payload: encoded_payload,
        token_id,
        incoming_message_pda: *incoming_message_pda,
        incoming_message_account_data: incoming_message_account_data.clone(),
    };

    let accounts_config = ExecuteTestAccounts {
        core_accounts: vec![
            (token_mint_pda, existing_token_mint_account),
            (token_manager_ata, new_empty_account()),
        ],
        extra_accounts: vec![
            (payer, payer_account), // deployer same as payer
        ],
        extra_account_metas: link_token_extra_accounts(payer),
    };

    let checks = vec![Check::success()];

    let test_result = execute_its_instruction(context, params, accounts_config, checks);

    assert!(test_result.result.program_result.is_ok());

    let token_manager_account = test_result.result.get_account(&token_manager_pda).unwrap();
    let token_manager =
        TokenManager::try_deserialize(&mut token_manager_account.data.as_ref()).unwrap();

    assert_eq!(token_manager.token_id, token_id);
}

#[test]
fn test_reject_execute_link_token_with_invalid_token_manager_type() {
    // Step 1: Setup gateway with real signers
    let (mut setup, verifier_leaves, verifier_merkle_tree, secret_key_1, secret_key_2) =
        setup_test_with_real_signers();

    // Step 2: Initialize gateway
    let init_result = initialize_gateway(&setup);
    let (gateway_root_pda, _) = GatewayConfig::find_pda();
    let gateway_root_pda_account = init_result.get_account(&gateway_root_pda).unwrap();

    assert!(init_result.program_result.is_ok());

    // Step 3: Add ITS program to mollusk - USE the properly configured mollusk
    let program_id = solana_axelar_its::id();

    // Use the properly configured mollusk that has Token2022 and other programs
    let mut mollusk = initialize_mollusk();

    // We still need to add the gateway program since initialize_mollusk doesn't include it for execution tests
    mollusk.add_program(
        &GATEWAY_PROGRAM_ID,
        "../../target/deploy/solana_axelar_gateway",
        &solana_sdk::bpf_loader_upgradeable::id(),
    );

    // Update setup to use our properly configured mollusk
    setup.mollusk = mollusk;

    // Step 4: Initialize ITS service
    let (payer, payer_account) = new_test_account();
    let (operator, operator_account) = new_test_account();

    let chain_name = "solana".to_owned();
    let its_hub_address = "0x123456789abcdef".to_owned();

    // Use our configured mollusk for ITS service initialization too
    let (its_root_pda, its_root_account) = init_its_service_with_ethereum_trusted(
        &setup.mollusk, // Now this mollusk has Token2022 properly configured
        payer,
        &payer_account,
        payer,
        operator,
        &operator_account,
        chain_name.clone(),
        its_hub_address.clone(),
    );

    // Step 5: Create existing token mint (key difference from deploy test)
    let mint_authority = payer; // Use payer as mint authority for simplicity
    let (existing_token_mint, existing_token_mint_account) = create_test_mint(mint_authority);

    // Step 6: Create token linking parameters
    let salt = [1u8; 32];
    let token_id = interchain_token_id(&payer, &salt);
    let token_manager_type = 0u8; // NativeInterchain type
    let source_token_address = existing_token_mint.to_bytes().to_vec();
    let destination_token_address = existing_token_mint.to_bytes().to_vec();
    let link_params = vec![]; // No additional params (no operator)

    // Step 7: Create the GMP payload
    let link_payload = LinkToken {
        selector: alloy_primitives::U256::from(5), // MESSAGE_TYPE_ID for LinkToken
        token_id: alloy_primitives::FixedBytes::from(token_id),
        token_manager_type: alloy_primitives::U256::from(token_manager_type),
        source_token_address: alloy_primitives::Bytes::from(source_token_address),
        destination_token_address: alloy_primitives::Bytes::from(destination_token_address),
        link_params: alloy_primitives::Bytes::from(link_params.clone()),
    };

    // Wrap in ReceiveFromHub payload
    let receive_from_hub_payload = ReceiveFromHub {
        selector: alloy_primitives::U256::from(4), // MESSAGE_TYPE_ID for ReceiveFromHub
        source_chain: "ethereum".to_owned(),
        payload: GMPPayload::LinkToken(link_payload).encode().into(),
    };

    let gmp_payload = GMPPayload::ReceiveFromHub(receive_from_hub_payload);
    let encoded_payload = gmp_payload.encode();
    let payload_hash = keccak::hashv(&[&encoded_payload]).to_bytes();

    // Step 8: Create test message
    let mut message = create_test_message(
        "ethereum",
        "link_token_123",
        &program_id.to_string(),
        payload_hash,
    );

    // Override the source_address to match its_hub_address
    message.source_address = its_hub_address.clone();

    // Step 9: Approve message on gateway
    let incoming_messages = approve_messages_on_gateway(
        &setup,
        vec![message.clone()],
        init_result.clone(),
        &secret_key_1,
        &secret_key_2,
        verifier_leaves,
        verifier_merkle_tree,
    );

    let (_, incoming_message_pda, incoming_message_account_data) = &incoming_messages[0];
    let (token_manager_pda, _) = TokenManager::find_pda(token_id, its_root_pda);

    // For link token, we use the existing mint, not a new PDA
    let token_mint_pda = existing_token_mint;

    let token_manager_ata = get_associated_token_address_with_program_id(
        &token_manager_pda,
        &token_mint_pda,
        &spl_token_2022::id(),
    );

    let context = ExecuteTestContext {
        mollusk: setup.mollusk,
        gateway_root: (gateway_root_pda, gateway_root_pda_account.clone()),
        its_root: (its_root_pda, its_root_account),
        payer: (payer, payer_account.clone()),
    };

    let params = ExecuteTestParams {
        message: message.clone(),
        payload: encoded_payload,
        token_id,
        incoming_message_pda: *incoming_message_pda,
        incoming_message_account_data: incoming_message_account_data.clone(),
    };

    let accounts_config = ExecuteTestAccounts {
        core_accounts: vec![
            (token_mint_pda, existing_token_mint_account),
            (token_manager_ata, new_empty_account()),
        ],
        extra_accounts: vec![
            (payer, payer_account), // deployer same as payer
        ],
        extra_account_metas: link_token_extra_accounts(payer),
    };

    let checks = vec![Check::err(
        anchor_lang::error::Error::from(ItsError::InvalidInstructionData).into(),
    )];

    let test_result = execute_its_instruction(context, params, accounts_config, checks);

    assert!(test_result.result.program_result.is_err());
}

#[test]
fn test_reject_execute_link_token_with_invalid_destination_token_address() {
    // Step 1: Setup gateway with real signers
    let (mut setup, verifier_leaves, verifier_merkle_tree, secret_key_1, secret_key_2) =
        setup_test_with_real_signers();

    // Step 2: Initialize gateway
    let init_result = initialize_gateway(&setup);
    let (gateway_root_pda, _) = GatewayConfig::find_pda();
    let gateway_root_pda_account = init_result.get_account(&gateway_root_pda).unwrap();

    assert!(init_result.program_result.is_ok());

    // Step 3: Add ITS program to mollusk - USE the properly configured mollusk
    let program_id = solana_axelar_its::id();

    // Use the properly configured mollusk that has Token2022 and other programs
    let mut mollusk = initialize_mollusk();

    // We still need to add the gateway program since initialize_mollusk doesn't include it for execution tests
    mollusk.add_program(
        &GATEWAY_PROGRAM_ID,
        "../../target/deploy/solana_axelar_gateway",
        &solana_sdk::bpf_loader_upgradeable::id(),
    );

    // Update setup to use our properly configured mollusk
    setup.mollusk = mollusk;

    // Step 4: Initialize ITS service
    let (payer, payer_account) = new_test_account();
    let (operator, operator_account) = new_test_account();

    let chain_name = "solana".to_owned();
    let its_hub_address = "0x123456789abcdef".to_owned();

    // Use our configured mollusk for ITS service initialization too
    let (its_root_pda, its_root_account) = init_its_service_with_ethereum_trusted(
        &setup.mollusk, // Now this mollusk has Token2022 properly configured
        payer,
        &payer_account,
        payer,
        operator,
        &operator_account,
        chain_name.clone(),
        its_hub_address.clone(),
    );

    // Step 5: Create existing token mint (key difference from deploy test)
    let mint_authority = payer; // Use payer as mint authority for simplicity
    let (existing_token_mint, existing_token_mint_account) = create_test_mint(mint_authority);

    // Step 6: Create token linking parameters
    let salt = [1u8; 32];
    let token_id = interchain_token_id(&payer, &salt);
    let token_manager_type = 1u8; // LockUnlock type
    let source_token_address = existing_token_mint.to_bytes().to_vec();
    let destination_token_address = Pubkey::new_unique().to_bytes().to_vec(); // invalid destination address
    let link_params = vec![]; // No additional params (no operator)

    // Step 7: Create the GMP payload
    let link_payload = LinkToken {
        selector: alloy_primitives::U256::from(5), // MESSAGE_TYPE_ID for LinkToken
        token_id: alloy_primitives::FixedBytes::from(token_id),
        token_manager_type: alloy_primitives::U256::from(token_manager_type),
        source_token_address: alloy_primitives::Bytes::from(source_token_address),
        destination_token_address: alloy_primitives::Bytes::from(destination_token_address),
        link_params: alloy_primitives::Bytes::from(link_params.clone()),
    };

    // Wrap in ReceiveFromHub payload
    let receive_from_hub_payload = ReceiveFromHub {
        selector: alloy_primitives::U256::from(4), // MESSAGE_TYPE_ID for ReceiveFromHub
        source_chain: "ethereum".to_owned(),
        payload: GMPPayload::LinkToken(link_payload).encode().into(),
    };

    let gmp_payload = GMPPayload::ReceiveFromHub(receive_from_hub_payload);
    let encoded_payload = gmp_payload.encode();
    let payload_hash = keccak::hashv(&[&encoded_payload]).to_bytes();

    // Step 8: Create test message
    let mut message = create_test_message(
        "ethereum",
        "link_token_123",
        &program_id.to_string(),
        payload_hash,
    );

    // Override the source_address to match its_hub_address
    message.source_address = its_hub_address.clone();

    // Step 9: Approve message on gateway
    let incoming_messages = approve_messages_on_gateway(
        &setup,
        vec![message.clone()],
        init_result.clone(),
        &secret_key_1,
        &secret_key_2,
        verifier_leaves,
        verifier_merkle_tree,
    );

    let (_, incoming_message_pda, incoming_message_account_data) = &incoming_messages[0];
    let (token_manager_pda, _) = TokenManager::find_pda(token_id, its_root_pda);

    // For link token, we use the existing mint, not a new PDA
    let token_mint_pda = existing_token_mint;

    let token_manager_ata = get_associated_token_address_with_program_id(
        &token_manager_pda,
        &token_mint_pda,
        &spl_token_2022::id(),
    );

    let context = ExecuteTestContext {
        mollusk: setup.mollusk,
        gateway_root: (gateway_root_pda, gateway_root_pda_account.clone()),
        its_root: (its_root_pda, its_root_account),
        payer: (payer, payer_account.clone()),
    };

    let params = ExecuteTestParams {
        message: message.clone(),
        payload: encoded_payload,
        token_id,
        incoming_message_pda: *incoming_message_pda,
        incoming_message_account_data: incoming_message_account_data.clone(),
    };

    let accounts_config = ExecuteTestAccounts {
        core_accounts: vec![
            (token_mint_pda, existing_token_mint_account),
            (token_manager_ata, new_empty_account()),
        ],
        extra_accounts: vec![
            (payer, payer_account), // deployer same as payer
        ],
        extra_account_metas: link_token_extra_accounts(payer),
    };

    let checks = vec![Check::err(
        anchor_lang::error::Error::from(ItsError::InvalidTokenMint).into(),
    )];

    let test_result = execute_its_instruction(context, params, accounts_config, checks);

    assert!(test_result.result.program_result.is_err());
}

#[test]
fn test_reject_execute_link_token_with_invalid_token_id() {
    // Step 1: Setup gateway with real signers
    let (mut setup, verifier_leaves, verifier_merkle_tree, secret_key_1, secret_key_2) =
        setup_test_with_real_signers();

    // Step 2: Initialize gateway
    let init_result = initialize_gateway(&setup);
    let (gateway_root_pda, _) = GatewayConfig::find_pda();
    let gateway_root_pda_account = init_result.get_account(&gateway_root_pda).unwrap();

    assert!(init_result.program_result.is_ok());

    // Step 3: Add ITS program to mollusk - USE the properly configured mollusk
    let program_id = solana_axelar_its::id();

    // Use the properly configured mollusk that has Token2022 and other programs
    let mut mollusk = initialize_mollusk();

    // We still need to add the gateway program since initialize_mollusk doesn't include it for execution tests
    mollusk.add_program(
        &GATEWAY_PROGRAM_ID,
        "../../target/deploy/solana_axelar_gateway",
        &solana_sdk::bpf_loader_upgradeable::id(),
    );

    // Update setup to use our properly configured mollusk
    setup.mollusk = mollusk;

    // Step 4: Initialize ITS service
    let (payer, payer_account) = new_test_account();
    let (operator, operator_account) = new_test_account();

    let chain_name = "solana".to_owned();
    let its_hub_address = "0x123456789abcdef".to_owned();

    // Use our configured mollusk for ITS service initialization too
    let (its_root_pda, its_root_account) = init_its_service_with_ethereum_trusted(
        &setup.mollusk, // Now this mollusk has Token2022 properly configured
        payer,
        &payer_account,
        payer,
        operator,
        &operator_account,
        chain_name.clone(),
        its_hub_address.clone(),
    );

    // Step 5: Create existing token mint (key difference from deploy test)
    let mint_authority = payer; // Use payer as mint authority for simplicity
    let (existing_token_mint, existing_token_mint_account) = create_test_mint(mint_authority);

    // Step 6: Create token linking parameters
    let salt = [1u8; 32];
    let token_id = interchain_token_id(&payer, &salt);
    let token_manager_type = 1u8; // LockUnlock type
    let source_token_address = existing_token_mint.to_bytes().to_vec();
    let destination_token_address = existing_token_mint.to_bytes().to_vec();
    let link_params = vec![]; // No additional params (no operator)

    let invalid_token_id = [2u8; 32];

    // Step 7: Create the GMP payload
    let link_payload = LinkToken {
        selector: alloy_primitives::U256::from(5), // MESSAGE_TYPE_ID for LinkToken
        token_id: alloy_primitives::FixedBytes::from(invalid_token_id),
        token_manager_type: alloy_primitives::U256::from(token_manager_type),
        source_token_address: alloy_primitives::Bytes::from(source_token_address),
        destination_token_address: alloy_primitives::Bytes::from(destination_token_address),
        link_params: alloy_primitives::Bytes::from(link_params.clone()),
    };

    // Wrap in ReceiveFromHub payload
    let receive_from_hub_payload = ReceiveFromHub {
        selector: alloy_primitives::U256::from(4), // MESSAGE_TYPE_ID for ReceiveFromHub
        source_chain: "ethereum".to_owned(),
        payload: GMPPayload::LinkToken(link_payload).encode().into(),
    };

    let gmp_payload = GMPPayload::ReceiveFromHub(receive_from_hub_payload);
    let encoded_payload = gmp_payload.encode();
    let payload_hash = keccak::hashv(&[&encoded_payload]).to_bytes();

    // Step 8: Create test message
    let mut message = create_test_message(
        "ethereum",
        "link_token_123",
        &program_id.to_string(),
        payload_hash,
    );

    // Override the source_address to match its_hub_address
    message.source_address = its_hub_address.clone();

    // Step 9: Approve message on gateway
    let incoming_messages = approve_messages_on_gateway(
        &setup,
        vec![message.clone()],
        init_result.clone(),
        &secret_key_1,
        &secret_key_2,
        verifier_leaves,
        verifier_merkle_tree,
    );

    let (_, incoming_message_pda, incoming_message_account_data) = &incoming_messages[0];
    let (token_manager_pda, _) = TokenManager::find_pda(token_id, its_root_pda);

    // For link token, we use the existing mint, not a new PDA
    let token_mint_pda = existing_token_mint;

    let token_manager_ata = get_associated_token_address_with_program_id(
        &token_manager_pda,
        &token_mint_pda,
        &spl_token_2022::id(),
    );

    let context = ExecuteTestContext {
        mollusk: setup.mollusk,
        gateway_root: (gateway_root_pda, gateway_root_pda_account.clone()),
        its_root: (its_root_pda, its_root_account),
        payer: (payer, payer_account.clone()),
    };

    let params = ExecuteTestParams {
        message: message.clone(),
        payload: encoded_payload,
        token_id, // Using valid token_id for helper function (mismatch with payload)
        incoming_message_pda: *incoming_message_pda,
        incoming_message_account_data: incoming_message_account_data.clone(),
    };

    let accounts_config = ExecuteTestAccounts {
        core_accounts: vec![
            (token_mint_pda, existing_token_mint_account),
            (token_manager_ata, new_empty_account()),
        ],
        extra_accounts: vec![
            (payer, payer_account), // deployer same as payer
        ],
        extra_account_metas: link_token_extra_accounts(payer),
    };

    let checks = vec![Check::err(
        anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::ConstraintSeeds).into(),
    )];

    let test_result = execute_its_instruction(context, params, accounts_config, checks);

    assert!(test_result.result.program_result.is_err());
}
