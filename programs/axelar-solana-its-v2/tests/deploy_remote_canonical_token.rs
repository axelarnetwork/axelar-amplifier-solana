use anchor_lang::AnchorSerialize;
use anchor_lang::InstructionData;
use anchor_lang::ToAccountMetas;
use anchor_spl::token_2022::spl_token_2022;
use axelar_solana_gateway_v2::seed_prefixes::GATEWAY_SEED;
use axelar_solana_gateway_v2::ID as GATEWAY_PROGRAM_ID;
use axelar_solana_gateway_v2_test_fixtures::initialize_gateway;
use axelar_solana_gateway_v2_test_fixtures::setup_test_with_real_signers;
use axelar_solana_its_v2::state::TokenManager;
use axelar_solana_its_v2_test_fixtures::init_gas_service;
use axelar_solana_its_v2_test_fixtures::init_its_service_with_ethereum_trusted;
use axelar_solana_its_v2_test_fixtures::initialize_mollusk;
use axelar_solana_its_v2_test_fixtures::register_canonical_interchain_token_helper;
use axelar_solana_its_v2_test_fixtures::setup_operator;
use mollusk_test_utils::setup_mollusk;
use solana_program::program_pack::Pack;
use solana_sdk::signature::Keypair;
use solana_sdk::signer::Signer;
use solana_sdk::{
    account::Account, native_token::LAMPORTS_PER_SOL, pubkey::Pubkey, system_program,
};

#[path = "initialize.rs"]
mod initialize;

#[test]
fn test_deploy_remote_canonical_interchain_token() {
    // Initialize gateway
    let (setup, _, _, _, _) = setup_test_with_real_signers();

    let init_result = initialize_gateway(&setup);
    assert!(init_result.program_result.is_ok());

    // Initialize gas service
    let gas_service_program_id = axelar_solana_gas_service_v2::id();
    let mut mollusk = setup_mollusk(&gas_service_program_id, "axelar_solana_gas_service_v2");

    let operator = Pubkey::new_unique();
    let operator_account = Account::new(1_000_000_000, 0, &system_program::ID);

    let (operator_pda, operator_pda_account) =
        setup_operator(&mut mollusk, operator, &operator_account);

    let (treasury_pda, treasury_pda_account) = init_gas_service(
        &mollusk,
        operator,
        &operator_account,
        operator_pda,
        &operator_pda_account,
    );

    let (gateway_root_pda, _) = Pubkey::find_program_address(&[GATEWAY_SEED], &GATEWAY_PROGRAM_ID);
    let gateway_root_pda_account = init_result.get_account(&gateway_root_pda).unwrap();

    let program_id = axelar_solana_its_v2::id();
    let mollusk = initialize::initialize_mollusk();

    let payer = Pubkey::new_unique();
    let payer_account = Account::new(10 * LAMPORTS_PER_SOL, 0, &system_program::ID);

    let deployer = Pubkey::new_unique();
    let deployer_account = Account::new(10 * LAMPORTS_PER_SOL, 0, &system_program::ID);

    let operator = Pubkey::new_unique();
    let operator_account = Account::new(1_000_000_000, 0, &system_program::ID);

    let chain_name = "solana".to_string();
    let its_hub_address = "0x123456789abcdef".to_string();

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
    let destination_chain = "ethereum".to_string();
    let gas_value = 0;
    let signing_pda_bump = {
        let (_, bump) = Pubkey::find_program_address(
            &[axelar_solana_gateway_v2::seed_prefixes::CALL_CONTRACT_SIGNING_SEED],
            &program_id,
        );
        bump
    };

    // Calculate required PDAs for deploy remote canonical token
    let token_id = axelar_solana_its_v2::utils::canonical_interchain_token_id(&mint_pubkey);
    let (token_manager_pda, _token_manager_bump) = TokenManager::find_pda(token_id, its_root_pda);

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
        name: "Test Canonical Token".to_string(),
        symbol: "TCT".to_string(),
        uri: "https://example.com".to_string(),
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
        &[axelar_solana_gas_service_v2::state::Treasury::SEED_PREFIX],
        &axelar_solana_gas_service_v2::id(),
    );

    // Get call contract signing PDA
    let (call_contract_signing_pda, _call_contract_signing_bump) = Pubkey::find_program_address(
        &[axelar_solana_gateway_v2::seed_prefixes::CALL_CONTRACT_SIGNING_SEED],
        &program_id,
    );

    // Get event authorities
    let (gateway_event_authority, _gateway_event_bump) =
        Pubkey::find_program_address(&[b"__event_authority"], &axelar_solana_gateway_v2::ID);

    let (gas_event_authority, _gas_event_bump) =
        Pubkey::find_program_address(&[b"__event_authority"], &axelar_solana_gas_service_v2::id());

    let (event_authority, event_authority_account, program_account) =
        mollusk_test_utils::get_event_authority_and_program_accounts(&program_id);

    // Create the deploy remote canonical instruction
    let deploy_remote_ix = solana_program::instruction::Instruction {
        program_id,
        accounts: axelar_solana_its_v2::accounts::DeployRemoteCanonicalInterchainToken {
            payer: deployer,
            token_mint: mint_pubkey,
            metadata_account: metadata_account_pda,
            token_manager_pda,
            gateway_root_pda,
            axelar_gateway_program: axelar_solana_gateway_v2::ID,
            gas_treasury: gas_treasury_pda,
            gas_service: axelar_solana_gas_service_v2::id(),
            system_program: system_program::ID,
            its_root_pda,
            call_contract_signing_pda,
            its_program: program_id,
            gateway_event_authority,
            gas_event_authority,
            //
            event_authority,
            program: program_id,
        }
        .to_account_metas(None),
        data: axelar_solana_its_v2::instruction::DeployRemoteCanonicalInterchainToken {
            destination_chain: destination_chain.clone(),
            gas_value,
            signing_pda_bump,
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
            axelar_solana_gateway_v2::ID,
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
            axelar_solana_gas_service_v2::id(),
            Account {
                lamports: 1,
                data: vec![],
                owner: solana_sdk::bpf_loader_upgradeable::ID,
                executable: true,
                rent_epoch: 0,
            },
        ),
        (
            system_program::ID,
            Account {
                lamports: 1,
                data: vec![],
                owner: solana_sdk::bpf_loader_upgradeable::ID,
                executable: true,
                rent_epoch: 0,
            },
        ),
        (its_root_pda, its_root_account.clone()),
        (
            call_contract_signing_pda,
            Account::new(0, 0, &system_program::ID),
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
            Account::new(0, 0, &system_program::ID),
        ),
        (gas_event_authority, Account::new(0, 0, &system_program::ID)),
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
