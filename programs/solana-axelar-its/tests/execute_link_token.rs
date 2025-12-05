#![cfg(test)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::indexing_slicing)]

use anchor_lang::AccountDeserialize;
use interchain_token_transfer_gmp::{GMPPayload, LinkToken, ReceiveFromHub};
use mollusk_svm::result::Check;
use relayer_discovery_test_fixtures::RelayerDiscoveryTestFixture;
use solana_axelar_gateway_test_fixtures::create_test_message;
use solana_axelar_its::ItsError;
use solana_axelar_its::{state::TokenManager, utils::interchain_token_id};
use solana_axelar_its_test_fixtures::{
    create_test_mint, init_its_relayer_transaction, init_its_service_with_ethereum_trusted,
    initialize_mollusk_with_programs, new_test_account,
};
use solana_sdk::account::Account;
use solana_sdk::keccak;

#[test]
fn execute_link_token() {
    // Step 1: Setup gateway with real signers
    let mut fixture = RelayerDiscoveryTestFixture::new();

    // Step 3: Add ITS program to mollusk - USE the properly configured mollusk
    let program_id = solana_axelar_its::id();
    let mollusk = initialize_mollusk_with_programs();

    // Update setup to use our properly configured mollusk
    fixture.setup.mollusk = mollusk;

    // Step 4: Initialize ITS service
    let (payer, payer_account) = new_test_account();
    let (operator, operator_account) = new_test_account();

    let chain_name = "solana".to_owned();
    let its_hub_address = "0x123456789abcdef".to_owned();

    // Use our configured mollusk for ITS service initialization too
    let (its_root_pda, its_root_account) = init_its_service_with_ethereum_trusted(
        &fixture.setup.mollusk, // Now this mollusk has Token2022 properly configured
        payer,
        &payer_account,
        payer,
        operator,
        &operator_account,
        chain_name.clone(),
        its_hub_address.clone(),
    );

    let (its_relayer_transaction_pda, its_relayer_transaction_account) =
        init_its_relayer_transaction(&fixture.setup.mollusk, payer, &payer_account);

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
        source_token_address: alloy_primitives::Bytes::from(source_token_address.clone()),
        destination_token_address: alloy_primitives::Bytes::from(destination_token_address.clone()),
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

    let execute_accounts = vec![
        (its_root_pda, its_root_account.clone()),
        (
            its_relayer_transaction_pda,
            its_relayer_transaction_account.clone(),
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
        (existing_token_mint, existing_token_mint_account),
    ];

    let checks = vec![Check::success()];

    let test_result = fixture.approve_and_execute_with_checks(
        &message,
        encoded_payload,
        execute_accounts,
        Some(vec![checks]),
    );

    assert!(test_result.is_ok(), "{test_result:?}");

    let test_result = test_result.unwrap();

    let (token_manager_pda, _) = TokenManager::find_pda(token_id, its_root_pda);
    let token_manager_account = test_result.get_account(&token_manager_pda).unwrap();
    let token_manager =
        TokenManager::try_deserialize(&mut token_manager_account.data.as_ref()).unwrap();

    assert_eq!(token_manager.token_id, token_id);
}

#[test]
fn reject_execute_link_token_with_invalid_token_manager_type() {
    // Step 1: Setup gateway with real signers
    let mut fixture = RelayerDiscoveryTestFixture::new();

    // Step 3: Add ITS program to mollusk - USE the properly configured mollusk
    let program_id = solana_axelar_its::id();
    let mollusk = initialize_mollusk_with_programs();

    // Update setup to use our properly configured mollusk
    fixture.setup.mollusk = mollusk;

    // Step 4: Initialize ITS service
    let (payer, payer_account) = new_test_account();
    let (operator, operator_account) = new_test_account();

    let chain_name = "solana".to_owned();
    let its_hub_address = "0x123456789abcdef".to_owned();

    // Use our configured mollusk for ITS service initialization too
    let (its_root_pda, its_root_account) = init_its_service_with_ethereum_trusted(
        &fixture.setup.mollusk, // Now this mollusk has Token2022 properly configured
        payer,
        &payer_account,
        payer,
        operator,
        &operator_account,
        chain_name.clone(),
        its_hub_address.clone(),
    );

    let (its_relayer_transaction_pda, its_relayer_transaction_account) =
        init_its_relayer_transaction(&fixture.setup.mollusk, payer, &payer_account);

    // Step 5: Create existing token mint (key difference from deploy test)
    let mint_authority = payer; // Use payer as mint authority for simplicity
    let (existing_token_mint, existing_token_mint_account) = create_test_mint(mint_authority);

    // Step 6: Create token linking parameters
    let salt = [1u8; 32];
    let token_id = interchain_token_id(&payer, &salt);
    let token_manager_type = 0u8; // LockUnlock type
    let source_token_address = existing_token_mint.to_bytes().to_vec();
    let destination_token_address = existing_token_mint.to_bytes().to_vec();
    let link_params = vec![]; // No additional params (no operator)

    // Step 7: Create the GMP payload
    let link_payload = LinkToken {
        selector: alloy_primitives::U256::from(5), // MESSAGE_TYPE_ID for LinkToken
        token_id: alloy_primitives::FixedBytes::from(token_id),
        token_manager_type: alloy_primitives::U256::from(token_manager_type),
        source_token_address: alloy_primitives::Bytes::from(source_token_address.clone()),
        destination_token_address: alloy_primitives::Bytes::from(destination_token_address.clone()),
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

    let execute_accounts = vec![
        (its_root_pda, its_root_account.clone()),
        (
            its_relayer_transaction_pda,
            its_relayer_transaction_account.clone(),
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
        (existing_token_mint, existing_token_mint_account),
    ];

    let checks = vec![Check::err(
        anchor_lang::error::Error::from(ItsError::InvalidInstructionData).into(),
    )];

    let test_result = fixture.approve_and_execute_with_checks(
        &message,
        encoded_payload,
        execute_accounts,
        Some(vec![checks]),
    );

    assert!(test_result.is_err(), "{test_result:?}");
}
