#![cfg(test)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::indexing_slicing)]

use anchor_lang::{solana_program, AccountDeserialize};
use anchor_spl::token_2022::spl_token_2022;
use interchain_token_transfer_gmp::{DeployInterchainToken, GMPPayload, ReceiveFromHub};
use mollusk_svm::result::{Check, ProgramResult};
use mpl_token_metadata::accounts::Metadata;
use relayer_discovery_test_fixtures::{RelayerDiscoveryFixtureError, RelayerDiscoveryTestFixture};
use solana_axelar_gateway_test_fixtures::{create_test_message, initialize_gateway};
use solana_axelar_its::ItsError;
use solana_axelar_its::{state::TokenManager, utils::interchain_token_id};
use solana_axelar_its_test_fixtures::init_its_relayer_transaction;
use solana_axelar_its_test_fixtures::new_test_account;
use solana_axelar_its_test_fixtures::{
    create_sysvar_instructions_data, get_token_mint_pda, init_its_service_with_ethereum_trusted,
    initialize_mollusk_with_programs,
};
use solana_program::program_pack::Pack;
use solana_sdk::{account::Account, keccak, pubkey::Pubkey};

#[test]
fn test_execute_deploy_interchain_token_success() {
    let mut fixture = RelayerDiscoveryTestFixture::new();
    // Step 1-4: Common setup - gateway, mollusk, and ITS service initialization

    let init_result = initialize_gateway(&fixture.setup);
    assert!(init_result.program_result.is_ok());

    let program_id = solana_axelar_its::id();
    let mollusk = initialize_mollusk_with_programs();
    fixture.setup.mollusk = mollusk;
    let setup = &fixture.setup;

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
    let (its_relayer_transaction_pda, its_relayer_transaction_account) =
        init_its_relayer_transaction(&setup.mollusk, payer, &payer_account);

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

    // Step 8-9: Create message and accounts.
    let mut message = create_test_message(
        "ethereum",
        "deploy_token_123",
        &program_id.to_string(),
        payload_hash,
    );
    message.source_address = its_hub_address.clone();

    let execute_accounts = vec![
        (its_root_pda, its_root_account.clone()),
        (
            its_relayer_transaction_pda,
            its_relayer_transaction_account.clone(),
        ),
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
        (
            solana_sdk::system_program::ID,
            Account {
                lamports: 0,
                data: vec![],
                owner: solana_sdk::native_loader::id(),
                executable: true,
                rent_epoch: 0,
            },
        ),
        mollusk_svm_programs_token::token2022::keyed_account(),
        mollusk_svm_programs_token::associated_token::keyed_account(),
        mollusk_svm::program::keyed_account_for_system_program(),
    ];

    // Step 10: Approve and execute
    let test_result =
        fixture.approve_and_execute(&message, encoded_payload.clone(), execute_accounts);

    assert!(
        test_result.is_ok(),
        "Execute instruction should succeed: {:?}",
        test_result,
    );

    let test_result = test_result.unwrap();

    // Step 11: Verify success and results
    let (token_manager_pda, _) = TokenManager::find_pda(token_id, its_root_pda);
    let (token_mint_pda, _) = get_token_mint_pda(token_id);

    let (metadata_account, _) = Metadata::find_pda(&token_mint_pda);

    assert!(
        test_result.program_result.is_ok(),
        "Execute instruction should succeed: {:?}",
        test_result.program_result
    );

    let token_manager_account = test_result.get_account(&token_manager_pda).unwrap();
    let token_manager =
        TokenManager::try_deserialize(&mut token_manager_account.data.as_ref()).unwrap();
    assert_eq!(token_manager.token_id, token_id);

    let token_mint_account = test_result.get_account(&token_mint_pda).unwrap();
    let token_mint = spl_token_2022::state::Mint::unpack(&token_mint_account.data).unwrap();
    assert_eq!(token_mint.mint_authority, Some(token_manager_pda).into());
    assert_eq!(token_mint.freeze_authority, Some(token_manager_pda).into());
    assert_eq!(token_mint.decimals, decimals);
    assert_eq!(token_mint.supply, 0, "Initial supply should be 0");
    assert!(token_mint.is_initialized);

    let metadata_acc = test_result.get_account(&metadata_account).unwrap();
    let metadata = mpl_token_metadata::accounts::Metadata::from_bytes(&metadata_acc.data).unwrap();
    assert_eq!(metadata.mint, token_mint_pda);
    assert_eq!(metadata.update_authority, token_manager_pda);

    let metadata_name = metadata.name.trim_end_matches('\0');
    let metadata_symbol = metadata.symbol.trim_end_matches('\0');
    assert_eq!(metadata_name, name);
    assert_eq!(metadata_symbol, symbol);
}

#[test]
fn test_reject_execute_deploy_interchain_token_with_large_metadata() {
    let mut fixture = RelayerDiscoveryTestFixture::new();
    // Step 1-4: Common setup - gateway, mollusk, and ITS service initialization

    let init_result = initialize_gateway(&fixture.setup);
    assert!(init_result.program_result.is_ok());

    let program_id = solana_axelar_its::id();
    let mollusk = initialize_mollusk_with_programs();
    fixture.setup.mollusk = mollusk;
    let setup = &fixture.setup;

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

    let (its_relayer_transaction_pda, its_relayer_transaction_account) =
        init_its_relayer_transaction(&setup.mollusk, payer, &payer_account);

    // Step 5-7: Create deployment parameters and payload
    let salt = [1u8; 32];
    let token_id = interchain_token_id(&payer, &salt);
    let name = "Test Token ".repeat(10).trim_end().to_owned(); // large name, should revert
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

    // Step 8-9: Create message and accounts.
    let mut message = create_test_message(
        "ethereum",
        "deploy_token_123",
        &program_id.to_string(),
        payload_hash,
    );
    message.source_address = its_hub_address.clone();

    let execute_accounts = vec![
        (its_root_pda, its_root_account.clone()),
        (
            its_relayer_transaction_pda,
            its_relayer_transaction_account.clone(),
        ),
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
        (
            solana_sdk::system_program::ID,
            Account {
                lamports: 0,
                data: vec![],
                owner: solana_sdk::native_loader::id(),
                executable: true,
                rent_epoch: 0,
            },
        ),
        mollusk_svm_programs_token::token2022::keyed_account(),
        mollusk_svm_programs_token::associated_token::keyed_account(),
        mollusk_svm::program::keyed_account_for_system_program(),
    ];

    let checks = vec![Check::err(
        anchor_lang::error::Error::from(ItsError::InvalidArgument).into(),
    )];

    // Step 10: Approve and execute
    let test_result = fixture.approve_and_execute_with_checks(
        &message,
        encoded_payload.clone(),
        execute_accounts,
        Some(vec![checks]),
    );

    assert!(test_result.is_err());
}

#[test]
fn test_execute_deploy_interchain_token_with_minter_success() {
    let mut fixture = RelayerDiscoveryTestFixture::new();
    // Step 1-4: Common setup - gateway, mollusk, and ITS service initialization

    let init_result = initialize_gateway(&fixture.setup);
    assert!(init_result.program_result.is_ok());

    let program_id = solana_axelar_its::id();
    let mollusk = initialize_mollusk_with_programs();
    fixture.setup.mollusk = mollusk;
    let setup = &fixture.setup;

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

    let (its_relayer_transaction_pda, its_relayer_transaction_account) =
        init_its_relayer_transaction(&setup.mollusk, payer, &payer_account);

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
        minter: alloy_primitives::Bytes::from(minter.clone()), // Empty bytes for no minter
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

    // Step 8-9: Create message and accounts.
    let mut message = create_test_message(
        "ethereum",
        "deploy_token_123",
        &program_id.to_string(),
        payload_hash,
    );
    message.source_address = its_hub_address.clone();

    let execute_accounts = vec![
        (its_root_pda, its_root_account.clone()),
        (
            its_relayer_transaction_pda,
            its_relayer_transaction_account.clone(),
        ),
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
        mollusk_svm_programs_token::token2022::keyed_account(),
        mollusk_svm_programs_token::associated_token::keyed_account(),
        mollusk_svm::program::keyed_account_for_system_program(),
    ];

    // Step 10: Approve and execute
    let test_result =
        fixture.approve_and_execute(&message, encoded_payload.clone(), execute_accounts);

    assert!(
        test_result.is_ok(),
        "Execute instruction should succeed: {:?}",
        test_result,
    );

    let test_result = test_result.unwrap();

    // Step 11: Verify success and results
    let (token_manager_pda, _) = TokenManager::find_pda(token_id, its_root_pda);
    let (token_mint_pda, _) = get_token_mint_pda(token_id);

    let (metadata_account, _) = Metadata::find_pda(&token_mint_pda);

    assert!(
        test_result.program_result.is_ok(),
        "Execute instruction should succeed: {:?}",
        test_result.program_result
    );

    let token_manager_account = test_result.get_account(&token_manager_pda).unwrap();
    let token_manager =
        TokenManager::try_deserialize(&mut token_manager_account.data.as_ref()).unwrap();
    assert_eq!(token_manager.token_id, token_id);

    let token_mint_account = test_result.get_account(&token_mint_pda).unwrap();
    let token_mint = spl_token_2022::state::Mint::unpack(&token_mint_account.data).unwrap();
    assert_eq!(token_mint.mint_authority, Some(token_manager_pda).into());
    assert_eq!(token_mint.freeze_authority, Some(token_manager_pda).into());
    assert_eq!(token_mint.decimals, decimals);
    assert_eq!(token_mint.supply, 0, "Initial supply should be 0");
    assert!(token_mint.is_initialized);

    let metadata_acc = test_result.get_account(&metadata_account).unwrap();
    let metadata = mpl_token_metadata::accounts::Metadata::from_bytes(&metadata_acc.data).unwrap();
    assert_eq!(metadata.mint, token_mint_pda);
    assert_eq!(metadata.update_authority, token_manager_pda);

    let metadata_name = metadata.name.trim_end_matches('\0');
    let metadata_symbol = metadata.symbol.trim_end_matches('\0');
    assert_eq!(metadata_name, name);
    assert_eq!(metadata_symbol, symbol);
}
