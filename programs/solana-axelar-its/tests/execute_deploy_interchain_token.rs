#![cfg(test)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::indexing_slicing)]

use anchor_lang::solana_program;
use anchor_lang::AccountDeserialize;
use anchor_spl::associated_token::get_associated_token_address_with_program_id;
use anchor_spl::token_2022::spl_token_2022;
use interchain_token_transfer_gmp::{DeployInterchainToken, GMPPayload, ReceiveFromHub};
use mollusk_svm::result::Check;
use mpl_token_metadata::accounts::Metadata;
use mpl_token_metadata::MAX_NAME_LENGTH;
use mpl_token_metadata::MAX_SYMBOL_LENGTH;
use solana_axelar_gateway::GatewayConfig;
use solana_axelar_gateway_test_fixtures::{
    approve_messages_on_gateway, create_test_message, initialize_gateway,
    setup_test_with_real_signers,
};
use solana_axelar_its::ItsError;
use solana_axelar_its::{state::TokenManager, utils::interchain_token_id};
use solana_axelar_its_test_fixtures::new_empty_account;
use solana_axelar_its_test_fixtures::new_test_account;
use solana_axelar_its_test_fixtures::{
    create_sysvar_instructions_data, deploy_interchain_token_extra_accounts,
    execute_its_instruction, get_token_mint_pda, init_its_service_with_ethereum_trusted,
    initialize_mollusk_with_programs, ExecuteTestAccounts, ExecuteTestContext, ExecuteTestParams,
};
use solana_program::program_pack::{IsInitialized, Pack};
use solana_sdk::{account::Account, keccak, pubkey::Pubkey};
use spl_token_2022::{extension::StateWithExtensions, state::Account as Token2022Account};

#[test]
fn test_execute_deploy_interchain_token_success() {
    // Step 1-4: Common setup - gateway, mollusk, and ITS service initialization
    let (mut setup, verifier_leaves, verifier_merkle_tree, secret_key_1, secret_key_2) =
        setup_test_with_real_signers();

    let init_result = initialize_gateway(&setup);
    let (gateway_root_pda, _) = GatewayConfig::find_pda();
    let gateway_root_pda_account = init_result.get_account(&gateway_root_pda).unwrap();
    assert!(init_result.program_result.is_ok());

    let program_id = solana_axelar_its::id();
    let mollusk = initialize_mollusk_with_programs();
    setup.mollusk = mollusk;

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

    // Step 5-7: Create deployment parameters and payload
    let salt = [1u8; 32];
    let token_id = interchain_token_id(&payer, &salt);
    let name = "Test Token".to_owned();
    let symbol = "TEST".to_owned();
    let decimals = 9u8;

    let deploy_payload = DeployInterchainToken {
        selector: alloy_primitives::U256::from(1), // MESSAGE_TYPE_ID for DeployInterchainToken
        token_id: alloy_primitives::FixedBytes::from(token_id),
        name: name.clone(),
        symbol: symbol.clone(),
        decimals,
        minter: alloy_primitives::Bytes::new(), // Empty bytes for no minter
    };

    let receive_from_hub_payload = ReceiveFromHub {
        selector: alloy_primitives::U256::from(4), // MESSAGE_TYPE_ID for ReceiveFromHub
        source_chain: "ethereum".to_owned(),
        payload: GMPPayload::DeployInterchainToken(deploy_payload)
            .encode()
            .into(),
    };

    let gmp_payload = GMPPayload::ReceiveFromHub(receive_from_hub_payload);
    let encoded_payload = gmp_payload.encode();
    let payload_hash = keccak::hashv(&[&encoded_payload]).to_bytes();

    // Step 8-9: Create and approve message
    let mut message = create_test_message(
        "ethereum",
        "deploy_token_123",
        &program_id.to_string(),
        payload_hash,
    );
    message.source_address = its_hub_address.clone();

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

    // Step 10: Prepare accounts for helper function
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

    // Step 11: Execute using helper function
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

    let checks = vec![Check::success()];
    let test_result = execute_its_instruction(context, params, accounts_config, checks);

    // Step 12: Verify success and results
    assert!(
        test_result.result.program_result.is_ok(),
        "Execute instruction should succeed: {:?}",
        test_result.result.program_result
    );

    let token_manager_account = test_result.result.get_account(&token_manager_pda).unwrap();
    let token_manager =
        TokenManager::try_deserialize(&mut token_manager_account.data.as_ref()).unwrap();
    assert_eq!(token_manager.token_id, token_id);

    let token_mint_account = test_result.result.get_account(&token_mint_pda).unwrap();
    let token_mint = spl_token_2022::state::Mint::unpack(&token_mint_account.data).unwrap();
    assert_eq!(token_mint.mint_authority, Some(token_manager_pda).into());
    assert_eq!(token_mint.freeze_authority, Some(token_manager_pda).into());
    assert_eq!(token_mint.decimals, decimals);
    assert_eq!(token_mint.supply, 0, "Initial supply should be 0");
    assert!(token_mint.is_initialized);

    let metadata_acc = test_result.result.get_account(&metadata_account).unwrap();
    let metadata = mpl_token_metadata::accounts::Metadata::from_bytes(&metadata_acc.data).unwrap();
    assert_eq!(metadata.mint, token_mint_pda);
    assert_eq!(metadata.update_authority, token_manager_pda);

    let metadata_name = metadata.name.trim_end_matches('\0');
    let metadata_symbol = metadata.symbol.trim_end_matches('\0');
    assert_eq!(metadata_name, name);
    assert_eq!(metadata_symbol, symbol);

    let deployer_ata_account = test_result.result.get_account(&deployer_ata).unwrap();
    let deployer_ata_data =
        StateWithExtensions::<Token2022Account>::unpack(&deployer_ata_account.data).unwrap();
    assert_eq!(deployer_ata_data.base.mint, token_mint_pda);
    assert_eq!(deployer_ata_data.base.owner, payer);
    assert_eq!(deployer_ata_data.base.amount, 0);
    assert!(deployer_ata_data.base.is_initialized());
}

#[test]
#[allow(clippy::string_slice)]
fn test_execute_deploy_interchain_token_with_large_metadata() {
    // Step 1-4: Common setup - gateway, mollusk, and ITS service initialization
    let (mut setup, verifier_leaves, verifier_merkle_tree, secret_key_1, secret_key_2) =
        setup_test_with_real_signers();

    let init_result = initialize_gateway(&setup);
    let (gateway_root_pda, _) = GatewayConfig::find_pda();
    let gateway_root_pda_account = init_result.get_account(&gateway_root_pda).unwrap();
    assert!(init_result.program_result.is_ok());

    let program_id = solana_axelar_its::id();
    let mollusk = initialize_mollusk_with_programs();
    setup.mollusk = mollusk;

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

    // Step 5-7: Create deployment parameters and payload
    let salt = [1u8; 32];
    let token_id = interchain_token_id(&payer, &salt);
    let name = "Test Token ".repeat(10).trim_end().to_owned(); // large name, should revert
    let symbol = "TEST".repeat(10).to_owned();
    let decimals = 9u8;

    let deploy_payload = DeployInterchainToken {
        selector: alloy_primitives::U256::from(1), // MESSAGE_TYPE_ID for DeployInterchainToken
        token_id: alloy_primitives::FixedBytes::from(token_id),
        name: name.clone(),
        symbol: symbol.clone(),
        decimals,
        minter: alloy_primitives::Bytes::new(), // Empty bytes for no minter
    };

    let receive_from_hub_payload = ReceiveFromHub {
        selector: alloy_primitives::U256::from(4), // MESSAGE_TYPE_ID for ReceiveFromHub
        source_chain: "ethereum".to_owned(),
        payload: GMPPayload::DeployInterchainToken(deploy_payload)
            .encode()
            .into(),
    };

    let gmp_payload = GMPPayload::ReceiveFromHub(receive_from_hub_payload);
    let encoded_payload = gmp_payload.encode();
    let payload_hash = keccak::hashv(&[&encoded_payload]).to_bytes();

    // Step 8-9: Create and approve message
    let mut message = create_test_message(
        "ethereum",
        "deploy_token_123",
        &program_id.to_string(),
        payload_hash,
    );
    message.source_address = its_hub_address.clone();

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

    // Step 10: Prepare accounts for helper function
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

    // Step 11: Execute using helper function
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
            (token_mint_pda, new_empty_account()),
            (token_manager_ata, new_empty_account()),
        ],
        extra_accounts: vec![
            (deployer_ata, new_empty_account()),
            (payer, payer_account), // deployer is also payer
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

    let checks = vec![Check::success()];
    let test_result = execute_its_instruction(context, params, accounts_config, checks);

    let created_metadata_account = test_result.result.get_account(&metadata_account).unwrap();
    let created_metadata_account = Metadata::from_bytes(&created_metadata_account.data)
        .expect("should be valid metadata account");

    assert_eq!(created_metadata_account.name, name[..MAX_NAME_LENGTH]);
    assert_eq!(created_metadata_account.symbol, symbol[..MAX_SYMBOL_LENGTH]);
}

#[test]
fn test_reject_execute_deploy_interchain_token_with_mismatched_minter() {
    // Step 1-4: Common setup - gateway, mollusk, and ITS service initialization
    let (mut setup, verifier_leaves, verifier_merkle_tree, secret_key_1, secret_key_2) =
        setup_test_with_real_signers();

    let init_result = initialize_gateway(&setup);
    let (gateway_root_pda, _) = GatewayConfig::find_pda();
    let gateway_root_pda_account = init_result.get_account(&gateway_root_pda).unwrap();
    assert!(init_result.program_result.is_ok());

    let program_id = solana_axelar_its::id();
    let mollusk = initialize_mollusk_with_programs();
    setup.mollusk = mollusk;

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

    // Step 5-7: Create deployment parameters and payload
    let salt = [1u8; 32];
    let token_id = interchain_token_id(&payer, &salt);
    let name = "Test Token".to_owned();
    let symbol = "TEST".to_owned();
    let decimals = 9u8;

    let minter = Pubkey::new_unique().to_bytes();

    let deploy_payload = DeployInterchainToken {
        selector: alloy_primitives::U256::from(1), // MESSAGE_TYPE_ID for DeployInterchainToken
        token_id: alloy_primitives::FixedBytes::from(token_id),
        name: name.clone(),
        symbol: symbol.clone(),
        decimals,
        minter: alloy_primitives::Bytes::from(minter),
    };

    let receive_from_hub_payload = ReceiveFromHub {
        selector: alloy_primitives::U256::from(4), // MESSAGE_TYPE_ID for ReceiveFromHub
        source_chain: "ethereum".to_owned(),
        payload: GMPPayload::DeployInterchainToken(deploy_payload)
            .encode()
            .into(),
    };

    let gmp_payload = GMPPayload::ReceiveFromHub(receive_from_hub_payload);
    let encoded_payload = gmp_payload.encode();
    let payload_hash = keccak::hashv(&[&encoded_payload]).to_bytes();

    // Step 8-9: Create and approve message
    let mut message = create_test_message(
        "ethereum",
        "deploy_token_123",
        &program_id.to_string(),
        payload_hash,
    );
    message.source_address = its_hub_address.clone();

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

    // Step 10: Prepare accounts for helper function
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

    // Step 11: Execute using helper function
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
            (token_mint_pda, new_empty_account()),
            (token_manager_ata, new_empty_account()),
        ],
        extra_accounts: vec![
            (deployer_ata, new_empty_account()),
            (payer, payer_account), // deployer is also payer
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

    let checks = vec![Check::err(
        anchor_lang::error::Error::from(ItsError::InvalidArgument).into(),
    )];
    let test_result = execute_its_instruction(context, params, accounts_config, checks);

    assert!(test_result.result.program_result.is_err());
}
