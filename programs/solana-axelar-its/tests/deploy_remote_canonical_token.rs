#![cfg(test)]
#![allow(clippy::too_many_lines)]

use anchor_lang::solana_program;
use anchor_lang::AnchorSerialize;
use anchor_lang::InstructionData;
use anchor_lang::ToAccountMetas;
use anchor_spl::token_2022::spl_token_2022;
use mollusk_svm::program::keyed_account_for_system_program;
use mollusk_test_utils::get_event_authority_and_program_accounts;
use mollusk_test_utils::setup_mollusk;
use solana_axelar_gateway::seed_prefixes::GATEWAY_SEED;
use solana_axelar_gateway::ID as GATEWAY_PROGRAM_ID;
use solana_axelar_gateway_test_fixtures::initialize_gateway;
use solana_axelar_gateway_test_fixtures::setup_test_with_real_signers;
use solana_axelar_its::state::TokenManager;
use solana_axelar_its_test_fixtures::init_gas_service;
use solana_axelar_its_test_fixtures::init_its_service_with_ethereum_trusted;
use solana_axelar_its_test_fixtures::initialize_mollusk;
use solana_axelar_its_test_fixtures::register_canonical_interchain_token_helper;
use solana_axelar_its_test_fixtures::setup_operator;
use solana_program::program_pack::Pack;
use solana_sdk::signature::Keypair;
use solana_sdk::signer::Signer;
use solana_sdk::{account::Account, native_token::LAMPORTS_PER_SOL, pubkey::Pubkey};

#[test]
fn test_deploy_remote_canonical_token() {
    // Initialize gateway
    let (setup, _, _, _, _) = setup_test_with_real_signers();

    let init_result = initialize_gateway(&setup);
    assert!(init_result.program_result.is_ok());

    // Initialize gas service
    let gas_service_program_id = solana_axelar_gas_service::id();
    let mut mollusk = setup_mollusk(&gas_service_program_id, "solana_axelar_gas_service");

    let operator = Pubkey::new_unique();
    let operator_account = Account::new(1_000_000_000, 0, &solana_sdk::system_program::ID);

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

    let program_id = solana_axelar_its::id();
    let mollusk = initialize_mollusk();

    let payer = Pubkey::new_unique();
    let payer_account = Account::new(10 * LAMPORTS_PER_SOL, 0, &solana_sdk::system_program::ID);

    let deployer = Pubkey::new_unique();
    let deployer_account = Account::new(10 * LAMPORTS_PER_SOL, 0, &solana_sdk::system_program::ID);

    let operator = Pubkey::new_unique();
    let operator_account = Account::new(1_000_000_000, 0, &solana_sdk::system_program::ID);

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
        payer,
        &payer_account,
        its_root_pda,
        &its_root_account,
        program_id,
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
    let (gas_treasury_pda, _gas_treasury_bump) = Pubkey::find_program_address(
        &[solana_axelar_gas_service::state::Treasury::SEED_PREFIX],
        &solana_axelar_gas_service::id(),
    );

    // Get call contract signing PDA
    let (call_contract_signing_pda, _call_contract_signing_bump) = Pubkey::find_program_address(
        &[solana_axelar_gateway::seed_prefixes::CALL_CONTRACT_SIGNING_SEED],
        &program_id,
    );

    // Get event authorities
    let (gateway_event_authority, _, _) =
        get_event_authority_and_program_accounts(&solana_axelar_gateway::ID);

    let (gas_event_authority, _, _) =
        get_event_authority_and_program_accounts(&solana_axelar_gas_service::ID);

    let (event_authority, event_authority_account, program_account) =
        mollusk_test_utils::get_event_authority_and_program_accounts(&program_id);

    // Create the deploy remote canonical instruction
    let deploy_remote_ix = solana_program::instruction::Instruction {
        program_id,
        accounts: solana_axelar_its::accounts::DeployRemoteCanonicalInterchainToken {
            payer: deployer,
            token_mint: mint_pubkey,
            metadata_account: metadata_account_pda,
            token_manager_pda,
            gateway_root_pda,
            gateway_program: solana_axelar_gateway::ID,
            system_program: solana_sdk::system_program::ID,
            its_root_pda,
            call_contract_signing_pda,
            gateway_event_authority,
            gas_service_accounts: solana_axelar_its::accounts::GasServiceAccounts {
                gas_treasury: gas_treasury_pda,
                gas_service: solana_axelar_gas_service::id(),
                gas_event_authority,
            },
            event_authority,
            program: program_id,
        }
        .to_account_metas(None),
        data: solana_axelar_its::instruction::DeployRemoteCanonicalInterchainToken {
            destination_chain: destination_chain.clone(),
            gas_value,
        }
        .data(),
    };

    // Set up accounts for deploy remote canonical instruction
    let deploy_accounts = vec![
        (deployer, deployer_account.clone()),
        (
            mint_pubkey,
            result.get_account(&mint_pubkey).unwrap().clone(),
        ),
        (metadata_account_pda, metadata_account),
        (
            token_manager_pda,
            result.get_account(&token_manager_pda).unwrap().clone(),
        ),
        (gateway_root_pda, gateway_root_pda_account.clone()),
        (
            solana_axelar_gateway::ID,
            Account {
                lamports: 1,
                data: vec![],
                owner: solana_sdk::bpf_loader_upgradeable::ID,
                executable: true,
                rent_epoch: 0,
            },
        ),
        (gas_treasury_pda, treasury_pda_account.clone()),
        (
            solana_axelar_gas_service::id(),
            Account {
                lamports: 1,
                data: vec![],
                owner: solana_sdk::bpf_loader_upgradeable::ID,
                executable: true,
                rent_epoch: 0,
            },
        ),
        keyed_account_for_system_program(),
        (its_root_pda, its_root_account.clone()),
        (
            call_contract_signing_pda,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
        (
            program_id,
            Account {
                lamports: 1,
                data: vec![],
                owner: solana_sdk::bpf_loader_upgradeable::ID,
                executable: true,
                rent_epoch: 0,
            },
        ),
        (
            gateway_event_authority,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
        (
            gas_event_authority,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
        // For event CPI
        (event_authority, event_authority_account),
        (program_id, program_account),
    ];

    let deploy_result = mollusk.process_instruction(&deploy_remote_ix, &deploy_accounts);

    assert!(
        deploy_result.program_result.is_ok(),
        "Deploy remote canonical token instruction should succeed: {:?}",
        deploy_result.program_result
    );
}
