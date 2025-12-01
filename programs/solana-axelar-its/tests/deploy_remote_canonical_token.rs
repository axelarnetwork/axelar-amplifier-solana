#![cfg(test)]
#![allow(clippy::too_many_lines)]

use anchor_lang::solana_program;
use anchor_lang::AnchorSerialize;
use anchor_spl::token_2022::spl_token_2022;
use mollusk_svm::result::Check;
use mollusk_test_utils::setup_mollusk;
use solana_axelar_gateway::seed_prefixes::GATEWAY_SEED;
use solana_axelar_gateway::ID as GATEWAY_PROGRAM_ID;
use solana_axelar_gateway_test_fixtures::initialize_gateway;
use solana_axelar_gateway_test_fixtures::setup_test_with_real_signers;
use solana_axelar_its::state::TokenManager;
use solana_axelar_its_test_fixtures::deploy_remote_canonical_token::deploy_remote_canonical_token_helper;
use solana_axelar_its_test_fixtures::init_gas_service;
use solana_axelar_its_test_fixtures::init_its_service_with_ethereum_trusted;
use solana_axelar_its_test_fixtures::initialize_mollusk_with_programs;
use solana_axelar_its_test_fixtures::new_test_account;
use solana_axelar_its_test_fixtures::register_canonical_interchain_token_helper;
use solana_axelar_its_test_fixtures::setup_operator;
use solana_axelar_its_test_fixtures::DeployRemoteCanonicalTokenContext;
use solana_program::program_pack::Pack;
use solana_sdk::signature::Keypair;
use solana_sdk::signer::Signer;
use solana_sdk::{account::Account, pubkey::Pubkey};

#[test]
fn deploy_remote_canonical_token() {
    // Initialize gateway
    let (setup, _, _, _, _) = setup_test_with_real_signers();

    let init_result = initialize_gateway(&setup);
    assert!(init_result.program_result.is_ok());

    // Initialize gas service
    let gas_service_program_id = solana_axelar_gas_service::id();
    let mut mollusk = setup_mollusk(&gas_service_program_id, "solana_axelar_gas_service");

    let (operator, operator_account) = new_test_account();

    let (operator_pda, operator_pda_account) =
        setup_operator(&mut mollusk, operator, &operator_account);

    let (_, treasury_pda_account) = init_gas_service(
        &mollusk,
        operator,
        &operator_account,
        operator_pda,
        &operator_pda_account,
    );

    let (gateway_root_pda, _) = Pubkey::find_program_address(&[GATEWAY_SEED], &GATEWAY_PROGRAM_ID);
    let gateway_root_pda_account = init_result.get_account(&gateway_root_pda).unwrap();

    let mollusk = initialize_mollusk_with_programs();

    let (payer, payer_account) = new_test_account();
    let (deployer, deployer_account) = new_test_account();
    let (operator, operator_account) = new_test_account();

    let chain_name = "solana".to_owned();
    let its_hub_address = "0x123456789abcdef".to_owned();

    let (its_root_pda, its_root_account) = init_its_service_with_ethereum_trusted(
        &mollusk,
        payer,
        &payer_account,
        payer,
        operator,
        &operator_account,
        chain_name.clone(),
        its_hub_address.clone(),
    );

    // Create a token mint (this would be an existing token we want to register as canonical)
    let mint_keypair = Keypair::new();
    let mint_pubkey = mint_keypair.pubkey();
    let mint_authority = Keypair::new();

    // Create a basic SPL token mint
    let mint_data = {
        let mut data = vec![0u8; spl_token_2022::state::Mint::LEN];
        let mint = spl_token_2022::state::Mint {
            mint_authority: Some(mint_authority.pubkey()).into(),
            supply: 1_000_000_000, // 1 billion tokens
            decimals: 9,
            is_initialized: true,
            freeze_authority: Some(mint_authority.pubkey()).into(),
        };
        spl_token_2022::state::Mint::pack(mint, &mut data).unwrap();
        data
    };

    let result = register_canonical_interchain_token_helper(
        &mollusk,
        mint_data,
        &mint_keypair,
        &mint_authority,
        (payer, payer_account),
        (its_root_pda, its_root_account.clone()),
        vec![Check::success()],
    );

    assert!(
        result.program_result.is_ok(),
        "Register canonical token instruction should succeed: {:?}",
        result.program_result
    );

    // Deploy remote canonical token
    let destination_chain = "ethereum".to_owned();
    let gas_value = 0;

    // Calculate required PDAs for deploy remote canonical token
    let token_id = solana_axelar_its::utils::canonical_interchain_token_id(&mint_pubkey);
    let (token_manager_pda, _) = TokenManager::find_pda(token_id, its_root_pda);

    // Get metadata account PDA
    let (metadata_account_pda, _metadata_bump) = Pubkey::find_program_address(
        &[
            b"metadata",
            mpl_token_metadata::programs::MPL_TOKEN_METADATA_ID.as_ref(),
            mint_pubkey.as_ref(),
        ],
        &mpl_token_metadata::programs::MPL_TOKEN_METADATA_ID,
    );

    // Create metadata account data
    let metadata = mpl_token_metadata::accounts::Metadata {
        key: mpl_token_metadata::types::Key::MetadataV1,
        update_authority: mint_authority.pubkey(),
        mint: mint_pubkey,
        name: "Test Canonical Token".to_owned(),
        symbol: "TCT".to_owned(),
        uri: "https://example.com".to_owned(),
        seller_fee_basis_points: 0,
        creators: None,
        primary_sale_happened: false,
        is_mutable: true,
        edition_nonce: None,
        token_standard: Some(mpl_token_metadata::types::TokenStandard::Fungible),
        collection: None,
        uses: None,
        collection_details: None,
        programmable_config: None,
    };

    let metadata_data = metadata.try_to_vec().unwrap();
    let metadata_account = Account {
        lamports: anchor_lang::prelude::Rent::default().minimum_balance(metadata_data.len()),
        data: metadata_data,
        owner: mpl_token_metadata::programs::MPL_TOKEN_METADATA_ID,
        executable: false,
        rent_epoch: 0,
    };

    // Get gas treasury PDA
    let (gas_treasury_pda, _) = Pubkey::find_program_address(
        &[solana_axelar_gas_service::state::Treasury::SEED_PREFIX],
        &solana_axelar_gas_service::id(),
    );

    let mint_account = result.get_account(&mint_pubkey).unwrap().clone();
    let token_manager_account = result.get_account(&token_manager_pda).unwrap().clone();

    let ctx = DeployRemoteCanonicalTokenContext::new(
        mollusk,
        (deployer, deployer_account),
        (mint_pubkey, mint_account),
        (metadata_account_pda, metadata_account),
        (token_manager_pda, token_manager_account),
        (gateway_root_pda, gateway_root_pda_account.clone()),
        (gas_treasury_pda, treasury_pda_account),
        (its_root_pda, its_root_account),
    );

    let deploy_result = deploy_remote_canonical_token_helper(
        ctx,
        destination_chain,
        gas_value,
        vec![Check::success()],
    );

    assert!(deploy_result.program_result.is_ok());
}

#[test]
fn reject_deploy_remote_canonical_token_with_mismatched_token_id() {
    // Initialize gateway
    let (setup, _, _, _, _) = setup_test_with_real_signers();

    let init_result = initialize_gateway(&setup);
    assert!(init_result.program_result.is_ok());

    // Initialize gas service
    let gas_service_program_id = solana_axelar_gas_service::id();
    let mut mollusk = setup_mollusk(&gas_service_program_id, "solana_axelar_gas_service");

    let (operator, operator_account) = new_test_account();
    let (operator_pda, operator_pda_account) =
        setup_operator(&mut mollusk, operator, &operator_account);

    let (_, treasury_pda_account) = init_gas_service(
        &mollusk,
        operator,
        &operator_account,
        operator_pda,
        &operator_pda_account,
    );

    let (gateway_root_pda, _) = Pubkey::find_program_address(&[GATEWAY_SEED], &GATEWAY_PROGRAM_ID);
    let gateway_root_pda_account = init_result.get_account(&gateway_root_pda).unwrap();

    let mollusk = initialize_mollusk_with_programs();

    let (payer, payer_account) = new_test_account();
    let (deployer, deployer_account) = new_test_account();
    let (operator, operator_account) = new_test_account();

    let chain_name = "solana".to_owned();
    let its_hub_address = "0x123456789abcdef".to_owned();

    let (its_root_pda, its_root_account) = init_its_service_with_ethereum_trusted(
        &mollusk,
        payer,
        &payer_account,
        payer,
        operator,
        &operator_account,
        chain_name.clone(),
        its_hub_address.clone(),
    );

    // Create a token mint (this would be an existing token we want to register as canonical)
    let mint_keypair = Keypair::new();
    let mint_pubkey = mint_keypair.pubkey();
    let mint_authority = Keypair::new();

    // Create a basic SPL token mint
    let mint_data = {
        let mut data = vec![0u8; spl_token_2022::state::Mint::LEN];
        let mint = spl_token_2022::state::Mint {
            mint_authority: Some(mint_authority.pubkey()).into(),
            supply: 1_000_000_000, // 1 billion tokens
            decimals: 9,
            is_initialized: true,
            freeze_authority: Some(mint_authority.pubkey()).into(),
        };
        spl_token_2022::state::Mint::pack(mint, &mut data).unwrap();
        data
    };

    let result = register_canonical_interchain_token_helper(
        &mollusk,
        mint_data,
        &mint_keypair,
        &mint_authority,
        (payer, payer_account),
        (its_root_pda, its_root_account.clone()),
        vec![Check::success()],
    );

    assert!(result.program_result.is_ok());

    // Deploy remote canonical token
    let destination_chain = "ethereum".to_owned();
    let gas_value = 0;

    // Calculate required PDAs for deploy remote canonical token
    let token_id = solana_axelar_its::utils::canonical_interchain_token_id(&mint_pubkey);
    let (token_manager_pda, _) = TokenManager::find_pda(token_id, its_root_pda);
    let token_manager_account = result.get_account(&token_manager_pda).unwrap().clone();

    // use an invalid token_id to create a mismatched token_manager_pda
    let invalid_token_id = [123u8; 32];
    let (mismatched_token_manager_pda, _) = TokenManager::find_pda(invalid_token_id, its_root_pda);

    // Get metadata account PDA
    let (metadata_account_pda, _metadata_bump) = Pubkey::find_program_address(
        &[
            b"metadata",
            mpl_token_metadata::programs::MPL_TOKEN_METADATA_ID.as_ref(),
            mint_pubkey.as_ref(),
        ],
        &mpl_token_metadata::programs::MPL_TOKEN_METADATA_ID,
    );

    // Create metadata account data
    let metadata = mpl_token_metadata::accounts::Metadata {
        key: mpl_token_metadata::types::Key::MetadataV1,
        update_authority: mint_authority.pubkey(),
        mint: mint_pubkey,
        name: "Test Canonical Token".to_owned(),
        symbol: "TCT".to_owned(),
        uri: "https://example.com".to_owned(),
        seller_fee_basis_points: 0,
        creators: None,
        primary_sale_happened: false,
        is_mutable: true,
        edition_nonce: None,
        token_standard: Some(mpl_token_metadata::types::TokenStandard::Fungible),
        collection: None,
        uses: None,
        collection_details: None,
        programmable_config: None,
    };

    let metadata_data = metadata.try_to_vec().unwrap();
    let metadata_account = Account {
        lamports: anchor_lang::prelude::Rent::default().minimum_balance(metadata_data.len()),
        data: metadata_data,
        owner: mpl_token_metadata::programs::MPL_TOKEN_METADATA_ID,
        executable: false,
        rent_epoch: 0,
    };

    // Get gas treasury PDA
    let (gas_treasury_pda, _gas_treasury_bump) = Pubkey::find_program_address(
        &[solana_axelar_gas_service::state::Treasury::SEED_PREFIX],
        &solana_axelar_gas_service::id(),
    );

    let mint_account = result.get_account(&mint_pubkey).unwrap().clone();

    let ctx = DeployRemoteCanonicalTokenContext::new(
        mollusk,
        (deployer, deployer_account),
        (mint_pubkey, mint_account),
        (metadata_account_pda, metadata_account),
        (mismatched_token_manager_pda, token_manager_account), // Use the mismatched PDA with correct account data
        (gateway_root_pda, gateway_root_pda_account.clone()),
        (gas_treasury_pda, treasury_pda_account),
        (its_root_pda, its_root_account),
    );

    let checks = vec![Check::err(
        anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::ConstraintSeeds).into(),
    )];

    let deploy_result =
        deploy_remote_canonical_token_helper(ctx, destination_chain, gas_value, checks);

    assert!(deploy_result.program_result.is_err());
}
