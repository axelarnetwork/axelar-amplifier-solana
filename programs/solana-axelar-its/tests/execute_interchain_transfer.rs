#![cfg(test)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::indexing_slicing)]

use anchor_lang::AccountDeserialize;
use anchor_spl::{
    associated_token::get_associated_token_address_with_program_id,
    token_2022::spl_token_2022::{self},
};
use interchain_token_transfer_gmp::{
    DeployInterchainToken, GMPPayload, InterchainTransfer, ReceiveFromHub,
};
use mollusk_svm::result::Check;
use mpl_token_metadata::accounts::Metadata;
use solana_axelar_gateway::GatewayConfig;
use solana_axelar_gateway_test_fixtures::{
    approve_messages_on_gateway, create_test_message, initialize_gateway,
    setup_test_with_real_signers,
};
use solana_axelar_its::{state::TokenManager, utils::interchain_token_id, ItsError};
use solana_axelar_its_test_fixtures::{
    create_sysvar_instructions_data, deploy_interchain_token_extra_accounts,
    execute_its_instruction, get_token_mint_pda, init_its_service_with_ethereum_trusted,
    initialize_mollusk_with_programs, interchain_transfer_extra_accounts, new_empty_account,
    new_test_account, ExecuteTestAccounts, ExecuteTestContext, ExecuteTestParams,
};
use solana_program::program_pack::{IsInitialized, Pack};
use solana_sdk::{account::Account, keccak, pubkey::Pubkey};
use spl_token_2022::{extension::StateWithExtensions, state::Account as Token2022Account};

#[test]
fn test_execute_interchain_transfer_success() {
    // Step 1: Setup gateway with real signers
    let (mut setup, verifier_leaves, verifier_merkle_tree, secret_key_1, secret_key_2) =
        setup_test_with_real_signers();

    // Step 2: Initialize gateway
    let init_result = initialize_gateway(&setup);
    let (gateway_root_pda, _) = GatewayConfig::find_pda();
    let gateway_root_pda_account = init_result.get_account(&gateway_root_pda).unwrap();

    assert!(init_result.program_result.is_ok());

    // Step 3: Add ITS program to mollusk
    let program_id = solana_axelar_its::id();
    let mollusk = initialize_mollusk_with_programs();
    setup.mollusk = mollusk;

    // Step 4: Initialize ITS service
    let (payer, payer_account) = new_test_account();
    let (operator, operator_account) = new_test_account();

    let chain_name = "solana".to_owned();
    let its_hub_address = "0x123456789abcdef".to_owned();

    let (its_root_pda, its_root_account) = init_its_service_with_ethereum_trusted(
        &setup.mollusk,
        payer,
        &payer_account,
        payer,
        operator,
        &operator_account,
        chain_name.clone(),
        its_hub_address.clone(),
    );

    // Step 5: First deploy a token using helper function with new mollusk
    let salt = [1u8; 32];
    let token_id = interchain_token_id(&payer, &salt);
    let name = "Test Token".to_owned();
    let symbol = "TEST".to_owned();
    let decimals = 9u8;

    // Create the deploy token GMP payload
    let deploy_payload = DeployInterchainToken {
        selector: alloy_primitives::U256::from(1), // MESSAGE_TYPE_ID for DeployInterchainToken
        token_id: alloy_primitives::FixedBytes::from(token_id),
        name: name.clone(),
        symbol: symbol.clone(),
        decimals,
        minter: alloy_primitives::Bytes::new(), // Empty bytes for no minter
    };

    // Wrap in ReceiveFromHub payload
    let deploy_receive_from_hub_payload = ReceiveFromHub {
        selector: alloy_primitives::U256::from(4), // MESSAGE_TYPE_ID for ReceiveFromHub
        source_chain: "ethereum".to_owned(),
        payload: GMPPayload::DeployInterchainToken(deploy_payload)
            .encode()
            .into(),
    };

    let deploy_gmp_payload = GMPPayload::ReceiveFromHub(deploy_receive_from_hub_payload);
    let deploy_encoded_payload = deploy_gmp_payload.encode();
    let deploy_payload_hash = keccak::hashv(&[&deploy_encoded_payload]).to_bytes();

    // Create test message for deploy
    let mut deploy_message = create_test_message(
        "ethereum",
        "deploy_token_123",
        &program_id.to_string(),
        deploy_payload_hash,
    );

    // Override the source_address to match its_hub_address
    deploy_message.source_address = its_hub_address.clone();

    // Approve deploy message on gateway
    let deploy_incoming_messages = approve_messages_on_gateway(
        &setup,
        vec![deploy_message.clone()],
        init_result.clone(),
        &secret_key_1,
        &secret_key_2,
        verifier_leaves.clone(),
        verifier_merkle_tree.clone(),
    );

    let (_, deploy_incoming_message_pda, deploy_incoming_message_account_data) =
        &deploy_incoming_messages[0];

    let (token_manager_pda, _) = TokenManager::find_pda(token_id, its_root_pda);
    let (token_mint_pda, _) = get_token_mint_pda(token_id);
    let token_manager_ata = get_associated_token_address_with_program_id(
        &token_manager_pda,
        &token_mint_pda,
        &spl_token_2022::id(),
    );
    let deployer_ata = get_associated_token_address_with_program_id(
        &payer,
        &token_mint_pda,
        &spl_token_2022::id(),
    );
    let (metadata_account, _) = Metadata::find_pda(&token_mint_pda);

    // Use new mollusk for deploy execution with helper function
    let deploy_mollusk = initialize_mollusk_with_programs();
    let deploy_context = ExecuteTestContext {
        mollusk: deploy_mollusk,
        gateway_root: (gateway_root_pda, gateway_root_pda_account.clone()),
        its_root: (its_root_pda, its_root_account.clone()),
        payer: (payer, payer_account.clone()),
    };

    let deploy_params = ExecuteTestParams {
        message: deploy_message.clone(),
        payload: deploy_encoded_payload,
        token_id,
        incoming_message_pda: *deploy_incoming_message_pda,
        incoming_message_account_data: deploy_incoming_message_account_data.clone(),
    };

    let deploy_accounts_config = ExecuteTestAccounts {
        core_accounts: vec![
            (token_mint_pda, new_empty_account()),
            (token_manager_ata, new_empty_account()),
        ],
        extra_accounts: vec![
            (deployer_ata, new_empty_account()),
            (payer, payer_account.clone()),
            (
                solana_sdk::sysvar::instructions::ID,
                Account {
                    lamports: 1_000_000_000,
                    data: create_sysvar_instructions_data(),
                    owner: solana_program::sysvar::id(),
                    executable: false,
                    rent_epoch: 0,
                },
            ),
            (
                mpl_token_metadata::ID,
                Account {
                    lamports: 1,
                    data: vec![],
                    owner: solana_sdk::bpf_loader_upgradeable::id(),
                    executable: true,
                    rent_epoch: 0,
                },
            ),
            (metadata_account, new_empty_account()),
        ],
        extra_account_metas: deploy_interchain_token_extra_accounts(
            deployer_ata,
            payer,
            metadata_account,
        ),
        token_manager_account: None,
    };

    let deploy_result = execute_its_instruction(
        deploy_context,
        deploy_params,
        deploy_accounts_config,
        vec![mollusk_svm::result::Check::success()],
    );

    // Verify deploy success
    assert!(deploy_result.result.program_result.is_ok());

    // Step 6: Create the interchain transfer using the deployed token
    let transfer_amount = 1_000_000u64;
    let destination_pubkey = payer; // Transfer to same user for simplicity
    let destination_address = destination_pubkey.to_bytes().to_vec(); // 32 bytes for Solana Pubkey
    let source_address = "ethereum_address_123".to_owned(); // some valid UTF-8 string

    let source_address_bytes = source_address.as_bytes().to_vec();

    let transfer_payload = InterchainTransfer {
        selector: alloy_primitives::U256::from(0), // MESSAGE_TYPE_ID for InterchainTransfer
        token_id: alloy_primitives::FixedBytes::from(token_id),
        source_address: alloy_primitives::Bytes::from(source_address_bytes.clone()),
        destination_address: alloy_primitives::Bytes::from(destination_address.clone()),
        amount: alloy_primitives::U256::from(transfer_amount),
        data: alloy_primitives::Bytes::new(), // Empty data for simple transfer
    };

    let transfer_receive_from_hub_payload = ReceiveFromHub {
        selector: alloy_primitives::U256::from(4),
        source_chain: "ethereum".to_owned(),
        payload: GMPPayload::InterchainTransfer(transfer_payload)
            .encode()
            .into(),
    };

    let transfer_gmp_payload = GMPPayload::ReceiveFromHub(transfer_receive_from_hub_payload);
    let transfer_encoded_payload = transfer_gmp_payload.encode();
    let transfer_payload_hash = keccak::hashv(&[&transfer_encoded_payload]).to_bytes();

    let mut transfer_message = create_test_message(
        "ethereum",
        "transfer_token_456",
        &program_id.to_string(),
        transfer_payload_hash,
    );
    transfer_message.source_address = its_hub_address;

    let transfer_incoming_messages = approve_messages_on_gateway(
        &setup,
        vec![transfer_message.clone()],
        init_result.clone(),
        &secret_key_1,
        &secret_key_2,
        verifier_leaves,
        verifier_merkle_tree,
    );

    let (_, transfer_incoming_message_pda, transfer_incoming_message_account_data) =
        &transfer_incoming_messages[0];

    let destination_ata = get_associated_token_address_with_program_id(
        &destination_pubkey,
        &token_mint_pda,
        &spl_token_2022::id(),
    );

    // Get updated accounts from deploy result
    let token_manager_account_after_deploy = deploy_result
        .result
        .get_account(&token_manager_pda)
        .unwrap();
    let token_mint_account_after_deploy =
        deploy_result.result.get_account(&token_mint_pda).unwrap();
    let token_manager_ata_after_deploy = deploy_result
        .result
        .get_account(&token_manager_ata)
        .unwrap();
    let deployer_ata_account_after_deploy =
        deploy_result.result.get_account(&deployer_ata).unwrap();

    // Use new mollusk for transfer execution with helper function
    let transfer_mollusk = initialize_mollusk_with_programs();
    let transfer_context = ExecuteTestContext {
        mollusk: transfer_mollusk,
        gateway_root: (gateway_root_pda, gateway_root_pda_account.clone()),
        its_root: (its_root_pda, its_root_account.clone()),
        payer: (payer, payer_account.clone()),
    };

    let transfer_params = ExecuteTestParams {
        message: transfer_message.clone(),
        payload: transfer_encoded_payload,
        token_id,
        incoming_message_pda: *transfer_incoming_message_pda,
        incoming_message_account_data: transfer_incoming_message_account_data.clone(),
    };

    let transfer_accounts_config = ExecuteTestAccounts {
        core_accounts: vec![
            (token_mint_pda, token_mint_account_after_deploy.clone()),
            (token_manager_ata, token_manager_ata_after_deploy.clone()),
        ],
        extra_accounts: vec![
            (destination_pubkey, payer_account.clone()),
            (destination_ata, deployer_ata_account_after_deploy.clone()),
        ],
        extra_account_metas: interchain_transfer_extra_accounts(
            destination_ata,
            destination_pubkey,
        ),
        token_manager_account: Some(token_manager_account_after_deploy.clone()),
    };

    let transfer_result = execute_its_instruction(
        transfer_context,
        transfer_params,
        transfer_accounts_config,
        vec![mollusk_svm::result::Check::success()],
    );

    assert!(transfer_result.result.program_result.is_ok());

    let token_manager_account = transfer_result
        .result
        .get_account(&token_manager_pda)
        .unwrap();
    let token_manager =
        TokenManager::try_deserialize(&mut token_manager_account.data.as_ref()).unwrap();
    assert_eq!(token_manager.token_id, token_id);

    // Verify that the destination got their tokens
    let destination_ata_account_after = transfer_result
        .result
        .get_account(&destination_ata)
        .unwrap();
    let destination_ata_data =
        StateWithExtensions::<Token2022Account>::unpack(&destination_ata_account_after.data)
            .unwrap();

    assert_eq!(destination_ata_data.base.amount, transfer_amount,);
    assert_eq!(destination_ata_data.base.mint, token_mint_pda,);
    assert_eq!(destination_ata_data.base.owner, destination_pubkey,);
    assert!(destination_ata_data.base.is_initialized());

    let token_mint_account_after = transfer_result.result.get_account(&token_mint_pda).unwrap();
    let token_mint_after =
        spl_token_2022::state::Mint::unpack(&token_mint_account_after.data).unwrap();

    assert_eq!(token_mint_after.supply, transfer_amount,);
}

#[test]
fn test_reject_execute_interchain_transfer_with_zero_amount() {
    // Step 1: Setup gateway with real signers
    let (mut setup, verifier_leaves, verifier_merkle_tree, secret_key_1, secret_key_2) =
        setup_test_with_real_signers();

    // Step 2: Initialize gateway
    let init_result = initialize_gateway(&setup);
    let (gateway_root_pda, _) = GatewayConfig::find_pda();
    let gateway_root_pda_account = init_result.get_account(&gateway_root_pda).unwrap();

    assert!(init_result.program_result.is_ok());

    // Step 3: Add ITS program to mollusk
    let program_id = solana_axelar_its::id();
    let mollusk = initialize_mollusk_with_programs();
    setup.mollusk = mollusk;

    // Step 4: Initialize ITS service
    let (payer, payer_account) = new_test_account();

    let (operator, operator_account) = new_test_account();

    let chain_name = "solana".to_owned();
    let its_hub_address = "0x123456789abcdef".to_owned();

    let (its_root_pda, its_root_account) = init_its_service_with_ethereum_trusted(
        &setup.mollusk,
        payer,
        &payer_account,
        payer,
        operator,
        &operator_account,
        chain_name.clone(),
        its_hub_address.clone(),
    );

    // Step 5: First deploy a token using helper function with new mollusk
    let salt = [1u8; 32];
    let token_id = interchain_token_id(&payer, &salt);
    let name = "Test Token".to_owned();
    let symbol = "TEST".to_owned();
    let decimals = 9u8;

    // Create the deploy token GMP payload
    let deploy_payload = DeployInterchainToken {
        selector: alloy_primitives::U256::from(1), // MESSAGE_TYPE_ID for DeployInterchainToken
        token_id: alloy_primitives::FixedBytes::from(token_id),
        name: name.clone(),
        symbol: symbol.clone(),
        decimals,
        minter: alloy_primitives::Bytes::new(), // Empty bytes for no minter
    };

    // Wrap in ReceiveFromHub payload
    let deploy_receive_from_hub_payload = ReceiveFromHub {
        selector: alloy_primitives::U256::from(4), // MESSAGE_TYPE_ID for ReceiveFromHub
        source_chain: "ethereum".to_owned(),
        payload: GMPPayload::DeployInterchainToken(deploy_payload)
            .encode()
            .into(),
    };

    let deploy_gmp_payload = GMPPayload::ReceiveFromHub(deploy_receive_from_hub_payload);
    let deploy_encoded_payload = deploy_gmp_payload.encode();
    let deploy_payload_hash = keccak::hashv(&[&deploy_encoded_payload]).to_bytes();

    // Create test message for deploy
    let mut deploy_message = create_test_message(
        "ethereum",
        "deploy_token_123",
        &program_id.to_string(),
        deploy_payload_hash,
    );

    // Override the source_address to match its_hub_address
    deploy_message.source_address = its_hub_address.clone();

    // Approve deploy message on gateway
    let deploy_incoming_messages = approve_messages_on_gateway(
        &setup,
        vec![deploy_message.clone()],
        init_result.clone(),
        &secret_key_1,
        &secret_key_2,
        verifier_leaves.clone(),
        verifier_merkle_tree.clone(),
    );

    let (_, deploy_incoming_message_pda, deploy_incoming_message_account_data) =
        &deploy_incoming_messages[0];

    let (token_manager_pda, _) = TokenManager::find_pda(token_id, its_root_pda);
    let (token_mint_pda, _) = get_token_mint_pda(token_id);
    let token_manager_ata = get_associated_token_address_with_program_id(
        &token_manager_pda,
        &token_mint_pda,
        &spl_token_2022::id(),
    );
    let deployer_ata = get_associated_token_address_with_program_id(
        &payer,
        &token_mint_pda,
        &spl_token_2022::id(),
    );
    let (metadata_account, _) = Metadata::find_pda(&token_mint_pda);

    // Use new mollusk for deploy execution with helper function
    let deploy_mollusk = initialize_mollusk_with_programs();
    let deploy_context = ExecuteTestContext {
        mollusk: deploy_mollusk,
        gateway_root: (gateway_root_pda, gateway_root_pda_account.clone()),
        its_root: (its_root_pda, its_root_account.clone()),
        payer: (payer, payer_account.clone()),
    };

    let deploy_params = ExecuteTestParams {
        message: deploy_message.clone(),
        payload: deploy_encoded_payload,
        token_id,
        incoming_message_pda: *deploy_incoming_message_pda,
        incoming_message_account_data: deploy_incoming_message_account_data.clone(),
    };

    let deploy_accounts_config = ExecuteTestAccounts {
        core_accounts: vec![
            (token_mint_pda, new_empty_account()),
            (token_manager_ata, new_empty_account()),
        ],
        extra_accounts: vec![
            (deployer_ata, new_empty_account()),
            (payer, payer_account.clone()),
            (
                solana_sdk::sysvar::instructions::ID,
                Account {
                    lamports: 1_000_000_000,
                    data: create_sysvar_instructions_data(),
                    owner: solana_program::sysvar::id(),
                    executable: false,
                    rent_epoch: 0,
                },
            ),
            (
                mpl_token_metadata::ID,
                Account {
                    lamports: 1,
                    data: vec![],
                    owner: solana_sdk::bpf_loader_upgradeable::id(),
                    executable: true,
                    rent_epoch: 0,
                },
            ),
            (metadata_account, new_empty_account()),
        ],
        extra_account_metas: deploy_interchain_token_extra_accounts(
            deployer_ata,
            payer,
            metadata_account,
        ),
        token_manager_account: None,
    };

    let deploy_result = execute_its_instruction(
        deploy_context,
        deploy_params,
        deploy_accounts_config,
        vec![mollusk_svm::result::Check::success()],
    );

    // Verify deploy success
    assert!(deploy_result.result.program_result.is_ok());

    // Step 6: Create the interchain transfer using the deployed token
    let transfer_amount = 0u64;
    let destination_pubkey = payer; // Transfer to same user for simplicity
    let destination_address = destination_pubkey.to_bytes().to_vec(); // 32 bytes for Solana Pubkey
    let source_address = "ethereum_address_123".to_owned(); // some valid UTF-8 string

    let source_address_bytes = source_address.as_bytes().to_vec();

    let transfer_payload = InterchainTransfer {
        selector: alloy_primitives::U256::from(0), // MESSAGE_TYPE_ID for InterchainTransfer
        token_id: alloy_primitives::FixedBytes::from(token_id),
        source_address: alloy_primitives::Bytes::from(source_address_bytes.clone()),
        destination_address: alloy_primitives::Bytes::from(destination_address.clone()),
        amount: alloy_primitives::U256::from(transfer_amount),
        data: alloy_primitives::Bytes::new(), // Empty data for simple transfer
    };

    let transfer_receive_from_hub_payload = ReceiveFromHub {
        selector: alloy_primitives::U256::from(4),
        source_chain: "ethereum".to_owned(),
        payload: GMPPayload::InterchainTransfer(transfer_payload)
            .encode()
            .into(),
    };

    let transfer_gmp_payload = GMPPayload::ReceiveFromHub(transfer_receive_from_hub_payload);
    let transfer_encoded_payload = transfer_gmp_payload.encode();
    let transfer_payload_hash = keccak::hashv(&[&transfer_encoded_payload]).to_bytes();

    let mut transfer_message = create_test_message(
        "ethereum",
        "transfer_token_456",
        &program_id.to_string(),
        transfer_payload_hash,
    );
    transfer_message.source_address = its_hub_address;

    let transfer_incoming_messages = approve_messages_on_gateway(
        &setup,
        vec![transfer_message.clone()],
        init_result.clone(),
        &secret_key_1,
        &secret_key_2,
        verifier_leaves,
        verifier_merkle_tree,
    );

    let (_, transfer_incoming_message_pda, transfer_incoming_message_account_data) =
        &transfer_incoming_messages[0];

    let destination_ata = get_associated_token_address_with_program_id(
        &destination_pubkey,
        &token_mint_pda,
        &spl_token_2022::id(),
    );

    // Get updated accounts from deploy result
    let token_manager_account_after_deploy = deploy_result
        .result
        .get_account(&token_manager_pda)
        .unwrap();
    let token_mint_account_after_deploy =
        deploy_result.result.get_account(&token_mint_pda).unwrap();
    let token_manager_ata_after_deploy = deploy_result
        .result
        .get_account(&token_manager_ata)
        .unwrap();
    let deployer_ata_account_after_deploy =
        deploy_result.result.get_account(&deployer_ata).unwrap();

    // Use new mollusk for transfer execution with helper function
    let transfer_mollusk = initialize_mollusk_with_programs();
    let transfer_context = ExecuteTestContext {
        mollusk: transfer_mollusk,
        gateway_root: (gateway_root_pda, gateway_root_pda_account.clone()),
        its_root: (its_root_pda, its_root_account.clone()),
        payer: (payer, payer_account.clone()),
    };

    let transfer_params = ExecuteTestParams {
        message: transfer_message.clone(),
        payload: transfer_encoded_payload,
        token_id,
        incoming_message_pda: *transfer_incoming_message_pda,
        incoming_message_account_data: transfer_incoming_message_account_data.clone(),
    };

    let transfer_accounts_config = ExecuteTestAccounts {
        core_accounts: vec![
            (token_mint_pda, token_mint_account_after_deploy.clone()),
            (token_manager_ata, token_manager_ata_after_deploy.clone()),
        ],
        extra_accounts: vec![
            (destination_pubkey, payer_account.clone()),
            (destination_ata, deployer_ata_account_after_deploy.clone()),
        ],
        extra_account_metas: interchain_transfer_extra_accounts(
            destination_ata,
            destination_pubkey,
        ),
        token_manager_account: Some(token_manager_account_after_deploy.clone()),
    };

    let checks = vec![Check::err(
        anchor_lang::error::Error::from(ItsError::InvalidAmount).into(),
    )];

    let transfer_result = execute_its_instruction(
        transfer_context,
        transfer_params,
        transfer_accounts_config,
        checks,
    );

    assert!(transfer_result.result.program_result.is_err());
}

#[test]
fn test_reject_execute_interchain_transfer_with_invalid_token_id() {
    // Step 1: Setup gateway with real signers
    let (mut setup, verifier_leaves, verifier_merkle_tree, secret_key_1, secret_key_2) =
        setup_test_with_real_signers();

    // Step 2: Initialize gateway
    let init_result = initialize_gateway(&setup);
    let (gateway_root_pda, _) = GatewayConfig::find_pda();
    let gateway_root_pda_account = init_result.get_account(&gateway_root_pda).unwrap();

    assert!(init_result.program_result.is_ok());

    // Step 3: Add ITS program to mollusk
    let program_id = solana_axelar_its::id();
    let mollusk = initialize_mollusk_with_programs();
    setup.mollusk = mollusk;

    // Step 4: Initialize ITS service
    let (payer, payer_account) = new_test_account();

    let (operator, operator_account) = new_test_account();

    let chain_name = "solana".to_owned();
    let its_hub_address = "0x123456789abcdef".to_owned();

    let (its_root_pda, its_root_account) = init_its_service_with_ethereum_trusted(
        &setup.mollusk,
        payer,
        &payer_account,
        payer,
        operator,
        &operator_account,
        chain_name.clone(),
        its_hub_address.clone(),
    );

    // Step 5: First deploy a token using helper function with new mollusk
    let salt = [1u8; 32];
    let token_id = interchain_token_id(&payer, &salt);
    let name = "Test Token".to_owned();
    let symbol = "TEST".to_owned();
    let decimals = 9u8;

    // Create the deploy token GMP payload
    let deploy_payload = DeployInterchainToken {
        selector: alloy_primitives::U256::from(1), // MESSAGE_TYPE_ID for DeployInterchainToken
        token_id: alloy_primitives::FixedBytes::from(token_id),
        name: name.clone(),
        symbol: symbol.clone(),
        decimals,
        minter: alloy_primitives::Bytes::new(), // Empty bytes for no minter
    };

    // Wrap in ReceiveFromHub payload
    let deploy_receive_from_hub_payload = ReceiveFromHub {
        selector: alloy_primitives::U256::from(4), // MESSAGE_TYPE_ID for ReceiveFromHub
        source_chain: "ethereum".to_owned(),
        payload: GMPPayload::DeployInterchainToken(deploy_payload)
            .encode()
            .into(),
    };

    let deploy_gmp_payload = GMPPayload::ReceiveFromHub(deploy_receive_from_hub_payload);
    let deploy_encoded_payload = deploy_gmp_payload.encode();
    let deploy_payload_hash = keccak::hashv(&[&deploy_encoded_payload]).to_bytes();

    // Create test message for deploy
    let mut deploy_message = create_test_message(
        "ethereum",
        "deploy_token_123",
        &program_id.to_string(),
        deploy_payload_hash,
    );

    // Override the source_address to match its_hub_address
    deploy_message.source_address = its_hub_address.clone();

    // Approve deploy message on gateway
    let deploy_incoming_messages = approve_messages_on_gateway(
        &setup,
        vec![deploy_message.clone()],
        init_result.clone(),
        &secret_key_1,
        &secret_key_2,
        verifier_leaves.clone(),
        verifier_merkle_tree.clone(),
    );

    let (_, deploy_incoming_message_pda, deploy_incoming_message_account_data) =
        &deploy_incoming_messages[0];

    let (token_manager_pda, _) = TokenManager::find_pda(token_id, its_root_pda);
    let (token_mint_pda, _) = get_token_mint_pda(token_id);
    let token_manager_ata = get_associated_token_address_with_program_id(
        &token_manager_pda,
        &token_mint_pda,
        &spl_token_2022::id(),
    );
    let deployer_ata = get_associated_token_address_with_program_id(
        &payer,
        &token_mint_pda,
        &spl_token_2022::id(),
    );
    let (metadata_account, _) = Metadata::find_pda(&token_mint_pda);

    // Use new mollusk for deploy execution with helper function
    let deploy_mollusk = initialize_mollusk_with_programs();
    let deploy_context = ExecuteTestContext {
        mollusk: deploy_mollusk,
        gateway_root: (gateway_root_pda, gateway_root_pda_account.clone()),
        its_root: (its_root_pda, its_root_account.clone()),
        payer: (payer, payer_account.clone()),
    };

    let deploy_params = ExecuteTestParams {
        message: deploy_message.clone(),
        payload: deploy_encoded_payload,
        token_id,
        incoming_message_pda: *deploy_incoming_message_pda,
        incoming_message_account_data: deploy_incoming_message_account_data.clone(),
    };

    let deploy_accounts_config = ExecuteTestAccounts {
        core_accounts: vec![
            (token_mint_pda, new_empty_account()),
            (token_manager_ata, new_empty_account()),
        ],
        extra_accounts: vec![
            (deployer_ata, new_empty_account()),
            (payer, payer_account.clone()),
            (
                solana_sdk::sysvar::instructions::ID,
                Account {
                    lamports: 1_000_000_000,
                    data: create_sysvar_instructions_data(),
                    owner: solana_program::sysvar::id(),
                    executable: false,
                    rent_epoch: 0,
                },
            ),
            (
                mpl_token_metadata::ID,
                Account {
                    lamports: 1,
                    data: vec![],
                    owner: solana_sdk::bpf_loader_upgradeable::id(),
                    executable: true,
                    rent_epoch: 0,
                },
            ),
            (metadata_account, new_empty_account()),
        ],
        extra_account_metas: deploy_interchain_token_extra_accounts(
            deployer_ata,
            payer,
            metadata_account,
        ),
        token_manager_account: None,
    };

    let deploy_result = execute_its_instruction(
        deploy_context,
        deploy_params,
        deploy_accounts_config,
        vec![mollusk_svm::result::Check::success()],
    );

    // Verify deploy success
    assert!(deploy_result.result.program_result.is_ok());

    // Step 6: Create the interchain transfer using the deployed token
    let transfer_amount = 1_000_000u64;
    let destination_pubkey = payer; // Transfer to same user for simplicity
    let destination_address = destination_pubkey.to_bytes().to_vec(); // 32 bytes for Solana Pubkey
    let source_address = "ethereum_address_123".to_owned(); // some valid UTF-8 string

    let source_address_bytes = source_address.as_bytes().to_vec();

    let invalid_token_id = [2u8; 32];

    let transfer_payload = InterchainTransfer {
        selector: alloy_primitives::U256::from(0), // MESSAGE_TYPE_ID for InterchainTransfer
        token_id: alloy_primitives::FixedBytes::from(invalid_token_id),
        source_address: alloy_primitives::Bytes::from(source_address_bytes.clone()),
        destination_address: alloy_primitives::Bytes::from(destination_address.clone()),
        amount: alloy_primitives::U256::from(transfer_amount),
        data: alloy_primitives::Bytes::new(), // Empty data for simple transfer
    };

    let transfer_receive_from_hub_payload = ReceiveFromHub {
        selector: alloy_primitives::U256::from(4),
        source_chain: "ethereum".to_owned(),
        payload: GMPPayload::InterchainTransfer(transfer_payload)
            .encode()
            .into(),
    };

    let transfer_gmp_payload = GMPPayload::ReceiveFromHub(transfer_receive_from_hub_payload);
    let transfer_encoded_payload = transfer_gmp_payload.encode();
    let transfer_payload_hash = keccak::hashv(&[&transfer_encoded_payload]).to_bytes();

    let mut transfer_message = create_test_message(
        "ethereum",
        "transfer_token_456",
        &program_id.to_string(),
        transfer_payload_hash,
    );
    transfer_message.source_address = its_hub_address;

    let transfer_incoming_messages = approve_messages_on_gateway(
        &setup,
        vec![transfer_message.clone()],
        init_result.clone(),
        &secret_key_1,
        &secret_key_2,
        verifier_leaves,
        verifier_merkle_tree,
    );

    let (_, transfer_incoming_message_pda, transfer_incoming_message_account_data) =
        &transfer_incoming_messages[0];

    let destination_ata = get_associated_token_address_with_program_id(
        &destination_pubkey,
        &token_mint_pda,
        &spl_token_2022::id(),
    );

    // Get updated accounts from deploy result
    let token_manager_account_after_deploy = deploy_result
        .result
        .get_account(&token_manager_pda)
        .unwrap();
    let token_mint_account_after_deploy =
        deploy_result.result.get_account(&token_mint_pda).unwrap();
    let token_manager_ata_after_deploy = deploy_result
        .result
        .get_account(&token_manager_ata)
        .unwrap();
    let deployer_ata_account_after_deploy =
        deploy_result.result.get_account(&deployer_ata).unwrap();

    // Use new mollusk for transfer execution with helper function
    let transfer_mollusk = initialize_mollusk_with_programs();
    let transfer_context = ExecuteTestContext {
        mollusk: transfer_mollusk,
        gateway_root: (gateway_root_pda, gateway_root_pda_account.clone()),
        its_root: (its_root_pda, its_root_account.clone()),
        payer: (payer, payer_account.clone()),
    };

    let wrong_token_id = [123u8; 32];

    let transfer_params = ExecuteTestParams {
        message: transfer_message.clone(),
        payload: transfer_encoded_payload,
        token_id: wrong_token_id, // Use the wrong token ID
        incoming_message_pda: *transfer_incoming_message_pda,
        incoming_message_account_data: transfer_incoming_message_account_data.clone(),
    };

    let transfer_accounts_config = ExecuteTestAccounts {
        core_accounts: vec![
            (token_mint_pda, token_mint_account_after_deploy.clone()),
            (token_manager_ata, token_manager_ata_after_deploy.clone()),
        ],
        extra_accounts: vec![
            (destination_pubkey, payer_account.clone()),
            (destination_ata, deployer_ata_account_after_deploy.clone()),
        ],
        extra_account_metas: interchain_transfer_extra_accounts(
            destination_ata,
            destination_pubkey,
        ),
        token_manager_account: Some(token_manager_account_after_deploy.clone()),
    };

    let checks = vec![Check::err(
        anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::ConstraintSeeds).into(),
    )];

    let transfer_result = execute_its_instruction(
        transfer_context,
        transfer_params,
        transfer_accounts_config,
        checks,
    );

    assert!(transfer_result.result.program_result.is_err());
}

#[test]
fn test_reject_execute_interchain_transfer_with_mismatched_destination() {
    // Step 1: Setup gateway with real signers
    let (mut setup, verifier_leaves, verifier_merkle_tree, secret_key_1, secret_key_2) =
        setup_test_with_real_signers();

    // Step 2: Initialize gateway
    let init_result = initialize_gateway(&setup);
    let (gateway_root_pda, _) = GatewayConfig::find_pda();
    let gateway_root_pda_account = init_result.get_account(&gateway_root_pda).unwrap();

    assert!(init_result.program_result.is_ok());

    // Step 3: Add ITS program to mollusk
    let program_id = solana_axelar_its::id();
    let mollusk = initialize_mollusk_with_programs();
    setup.mollusk = mollusk;

    // Step 4: Initialize ITS service
    let (payer, payer_account) = new_test_account();

    let (operator, operator_account) = new_test_account();

    let chain_name = "solana".to_owned();
    let its_hub_address = "0x123456789abcdef".to_owned();

    let (its_root_pda, its_root_account) = init_its_service_with_ethereum_trusted(
        &setup.mollusk,
        payer,
        &payer_account,
        payer,
        operator,
        &operator_account,
        chain_name.clone(),
        its_hub_address.clone(),
    );

    // Step 5: First deploy a token using helper function with new mollusk
    let salt = [1u8; 32];
    let token_id = interchain_token_id(&payer, &salt);
    let name = "Test Token".to_owned();
    let symbol = "TEST".to_owned();
    let decimals = 9u8;

    // Create the deploy token GMP payload
    let deploy_payload = DeployInterchainToken {
        selector: alloy_primitives::U256::from(1), // MESSAGE_TYPE_ID for DeployInterchainToken
        token_id: alloy_primitives::FixedBytes::from(token_id),
        name: name.clone(),
        symbol: symbol.clone(),
        decimals,
        minter: alloy_primitives::Bytes::new(), // Empty bytes for no minter
    };

    // Wrap in ReceiveFromHub payload
    let deploy_receive_from_hub_payload = ReceiveFromHub {
        selector: alloy_primitives::U256::from(4), // MESSAGE_TYPE_ID for ReceiveFromHub
        source_chain: "ethereum".to_owned(),
        payload: GMPPayload::DeployInterchainToken(deploy_payload)
            .encode()
            .into(),
    };

    let deploy_gmp_payload = GMPPayload::ReceiveFromHub(deploy_receive_from_hub_payload);
    let deploy_encoded_payload = deploy_gmp_payload.encode();
    let deploy_payload_hash = keccak::hashv(&[&deploy_encoded_payload]).to_bytes();

    // Create test message for deploy
    let mut deploy_message = create_test_message(
        "ethereum",
        "deploy_token_123",
        &program_id.to_string(),
        deploy_payload_hash,
    );

    // Override the source_address to match its_hub_address
    deploy_message.source_address = its_hub_address.clone();

    // Approve deploy message on gateway
    let deploy_incoming_messages = approve_messages_on_gateway(
        &setup,
        vec![deploy_message.clone()],
        init_result.clone(),
        &secret_key_1,
        &secret_key_2,
        verifier_leaves.clone(),
        verifier_merkle_tree.clone(),
    );

    let (_, deploy_incoming_message_pda, deploy_incoming_message_account_data) =
        &deploy_incoming_messages[0];

    let (token_manager_pda, _) = TokenManager::find_pda(token_id, its_root_pda);
    let (token_mint_pda, _) = get_token_mint_pda(token_id);
    let token_manager_ata = get_associated_token_address_with_program_id(
        &token_manager_pda,
        &token_mint_pda,
        &spl_token_2022::id(),
    );
    let deployer_ata = get_associated_token_address_with_program_id(
        &payer,
        &token_mint_pda,
        &spl_token_2022::id(),
    );
    let (metadata_account, _) = Metadata::find_pda(&token_mint_pda);

    // Use new mollusk for deploy execution with helper function
    let deploy_mollusk = initialize_mollusk_with_programs();
    let deploy_context = ExecuteTestContext {
        mollusk: deploy_mollusk,
        gateway_root: (gateway_root_pda, gateway_root_pda_account.clone()),
        its_root: (its_root_pda, its_root_account.clone()),
        payer: (payer, payer_account.clone()),
    };

    let deploy_params = ExecuteTestParams {
        message: deploy_message.clone(),
        payload: deploy_encoded_payload,
        token_id,
        incoming_message_pda: *deploy_incoming_message_pda,
        incoming_message_account_data: deploy_incoming_message_account_data.clone(),
    };

    let deploy_accounts_config = ExecuteTestAccounts {
        core_accounts: vec![
            (token_mint_pda, new_empty_account()),
            (token_manager_ata, new_empty_account()),
        ],
        extra_accounts: vec![
            (deployer_ata, new_empty_account()),
            (payer, payer_account.clone()),
            (
                solana_sdk::sysvar::instructions::ID,
                Account {
                    lamports: 1_000_000_000,
                    data: create_sysvar_instructions_data(),
                    owner: solana_program::sysvar::id(),
                    executable: false,
                    rent_epoch: 0,
                },
            ),
            (
                mpl_token_metadata::ID,
                Account {
                    lamports: 1,
                    data: vec![],
                    owner: solana_sdk::bpf_loader_upgradeable::id(),
                    executable: true,
                    rent_epoch: 0,
                },
            ),
            (metadata_account, new_empty_account()),
        ],
        extra_account_metas: deploy_interchain_token_extra_accounts(
            deployer_ata,
            payer,
            metadata_account,
        ),
        token_manager_account: None,
    };

    let deploy_result = execute_its_instruction(
        deploy_context,
        deploy_params,
        deploy_accounts_config,
        vec![mollusk_svm::result::Check::success()],
    );

    // Verify deploy success
    assert!(deploy_result.result.program_result.is_ok());

    // Step 6: Create the interchain transfer using the deployed token
    let transfer_amount = 1_000_000u64;
    let destination_pubkey = payer; // Transfer to same user for simplicity
    let destination_address = Pubkey::new_unique().to_bytes().to_vec();
    let source_address = "ethereum_address_123".to_owned(); // some valid UTF-8 string

    let source_address_bytes = source_address.as_bytes().to_vec();

    let transfer_payload = InterchainTransfer {
        selector: alloy_primitives::U256::from(0), // MESSAGE_TYPE_ID for InterchainTransfer
        token_id: alloy_primitives::FixedBytes::from(token_id),
        source_address: alloy_primitives::Bytes::from(source_address_bytes.clone()),
        destination_address: alloy_primitives::Bytes::from(destination_address.clone()),
        amount: alloy_primitives::U256::from(transfer_amount),
        data: alloy_primitives::Bytes::new(), // Empty data for simple transfer
    };

    let transfer_receive_from_hub_payload = ReceiveFromHub {
        selector: alloy_primitives::U256::from(4),
        source_chain: "ethereum".to_owned(),
        payload: GMPPayload::InterchainTransfer(transfer_payload)
            .encode()
            .into(),
    };

    let transfer_gmp_payload = GMPPayload::ReceiveFromHub(transfer_receive_from_hub_payload);
    let transfer_encoded_payload = transfer_gmp_payload.encode();
    let transfer_payload_hash = keccak::hashv(&[&transfer_encoded_payload]).to_bytes();

    let mut transfer_message = create_test_message(
        "ethereum",
        "transfer_token_456",
        &program_id.to_string(),
        transfer_payload_hash,
    );
    transfer_message.source_address = its_hub_address;

    let transfer_incoming_messages = approve_messages_on_gateway(
        &setup,
        vec![transfer_message.clone()],
        init_result.clone(),
        &secret_key_1,
        &secret_key_2,
        verifier_leaves,
        verifier_merkle_tree,
    );

    let (_, transfer_incoming_message_pda, transfer_incoming_message_account_data) =
        &transfer_incoming_messages[0];

    let destination_ata = get_associated_token_address_with_program_id(
        &destination_pubkey,
        &token_mint_pda,
        &spl_token_2022::id(),
    );

    // Get updated accounts from deploy result
    let token_manager_account_after_deploy = deploy_result
        .result
        .get_account(&token_manager_pda)
        .unwrap();
    let token_mint_account_after_deploy =
        deploy_result.result.get_account(&token_mint_pda).unwrap();
    let token_manager_ata_after_deploy = deploy_result
        .result
        .get_account(&token_manager_ata)
        .unwrap();
    let deployer_ata_account_after_deploy =
        deploy_result.result.get_account(&deployer_ata).unwrap();

    // Use new mollusk for transfer execution with helper function
    let transfer_mollusk = initialize_mollusk_with_programs();
    let transfer_context = ExecuteTestContext {
        mollusk: transfer_mollusk,
        gateway_root: (gateway_root_pda, gateway_root_pda_account.clone()),
        its_root: (its_root_pda, its_root_account.clone()),
        payer: (payer, payer_account.clone()),
    };

    let transfer_params = ExecuteTestParams {
        message: transfer_message.clone(),
        payload: transfer_encoded_payload,
        token_id,
        incoming_message_pda: *transfer_incoming_message_pda,
        incoming_message_account_data: transfer_incoming_message_account_data.clone(),
    };

    let transfer_accounts_config = ExecuteTestAccounts {
        core_accounts: vec![
            (token_mint_pda, token_mint_account_after_deploy.clone()),
            (token_manager_ata, token_manager_ata_after_deploy.clone()),
        ],
        extra_accounts: vec![
            (destination_pubkey, payer_account.clone()),
            (destination_ata, deployer_ata_account_after_deploy.clone()),
        ],
        extra_account_metas: interchain_transfer_extra_accounts(
            destination_ata,
            destination_pubkey,
        ),
        token_manager_account: Some(token_manager_account_after_deploy.clone()),
    };

    let checks = vec![Check::err(
        anchor_lang::error::Error::from(ItsError::InvalidDestinationAddressAccount).into(),
    )];

    let transfer_result = execute_its_instruction(
        transfer_context,
        transfer_params,
        transfer_accounts_config,
        checks,
    );

    assert!(transfer_result.result.program_result.is_err());
}
