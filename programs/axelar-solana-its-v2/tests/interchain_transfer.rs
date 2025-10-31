use anchor_lang::{InstructionData, ToAccountMetas};
use anchor_spl::associated_token::get_associated_token_address_with_program_id;
use anchor_spl::token_2022::spl_token_2022;
use anchor_spl::token_2022::spl_token_2022::extension::StateWithExtensions;
use anchor_spl::token_2022::spl_token_2022::state::Account as TokenAccount;
use axelar_solana_gateway_v2::seed_prefixes::{CALL_CONTRACT_SIGNING_SEED, GATEWAY_SEED};
use axelar_solana_gateway_v2::ID as GATEWAY_PROGRAM_ID;
use axelar_solana_gateway_v2_test_fixtures::initialize_gateway;
use axelar_solana_gateway_v2_test_fixtures::setup_test_with_real_signers;
use axelar_solana_its_v2::utils::canonical_interchain_token_id;
use axelar_solana_its_v2_test_fixtures::init_its_service_with_ethereum_trusted;
use axelar_solana_its_v2_test_fixtures::initialize_mollusk;
use axelar_solana_its_v2_test_fixtures::setup_operator;
use axelar_solana_its_v2_test_fixtures::{
    deploy_interchain_token_helper, DeployInterchainTokenContext,
};
use axelar_solana_its_v2_test_fixtures::{
    init_gas_service, register_canonical_interchain_token_helper,
};
use mollusk_svm::program::keyed_account_for_system_program;
use mollusk_test_utils::{get_event_authority_and_program_accounts, setup_mollusk};
use solana_sdk::program_pack::Pack;
use solana_sdk::rent::Rent;
use solana_sdk::signature::Keypair;
use solana_sdk::signer::Signer;
use solana_sdk::{
    account::Account, instruction::Instruction, native_token::LAMPORTS_PER_SOL, pubkey::Pubkey,
};

#[test]
fn test_interchain_transfer_mint_burn() {
    let (setup, _, _, _, _) = setup_test_with_real_signers();

    let init_result = initialize_gateway(&setup);
    assert!(init_result.program_result.is_ok());

    let gas_service_program_id = axelar_solana_gas_service_v2::id();
    let mut mollusk = setup_mollusk(&gas_service_program_id, "axelar_solana_gas_service_v2");

    let operator = Pubkey::new_unique();
    let operator_account = Account::new(1_000_000_000, 0, &solana_sdk::system_program::ID);

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
    let mollusk = initialize_mollusk();

    let payer = Pubkey::new_unique();
    let payer_account = Account::new(10 * LAMPORTS_PER_SOL, 0, &solana_sdk::system_program::ID);

    let deployer = Pubkey::new_unique();
    let deployer_account = Account::new(10 * LAMPORTS_PER_SOL, 0, &solana_sdk::system_program::ID);

    let operator = Pubkey::new_unique();
    let operator_account = Account::new(1_000_000_000, 0, &solana_sdk::system_program::ID);

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

    let salt = [1u8; 32];
    let name = "Test Token".to_string();
    let symbol = "TEST".to_string();
    let decimals = 9u8;
    let initial_supply = 1_000_000_000u64;

    let ctx = DeployInterchainTokenContext::new(
        mollusk,
        its_root_pda,
        its_root_account,
        deployer,
        deployer_account,
        program_id,
        payer,
        payer_account,
        None,
        None,
    );

    let (
        result,
        token_manager_pda,
        token_mint_pda,
        token_manager_ata,
        deployer_ata,
        _metadata_account,
        mollusk,
    ) = deploy_interchain_token_helper(
        salt,
        name.clone(),
        symbol.clone(),
        decimals,
        initial_supply,
        ctx,
    );

    assert!(result.program_result.is_ok());

    let source = deployer;
    let source_ata = deployer_ata;
    let token_id = axelar_solana_its_v2::utils::interchain_token_id(&deployer, &salt);
    let destination_chain = "ethereum".to_string();
    let destination_address = b"0x1234567890123456789012345678901234567890".to_vec();
    let transfer_amount = 1_000_000u64;
    let gas_value = 0u64;

    let (signing_pda, signing_pda_bump) =
        Pubkey::find_program_address(&[CALL_CONTRACT_SIGNING_SEED], &axelar_solana_its_v2::ID);

    let (gas_event_authority, _) =
        Pubkey::find_program_address(&[b"__event_authority"], &axelar_solana_gas_service_v2::ID);

    let (gateway_event_authority, _) =
        Pubkey::find_program_address(&[b"__event_authority"], &axelar_solana_gateway_v2::ID);

    let (its_event_authority, _event_authority_account, its_program_account) =
        get_event_authority_and_program_accounts(&program_id);

    let accounts = axelar_solana_its_v2::accounts::InterchainTransfer {
        payer,
        authority: source,
        its_root_pda,
        source_ata,
        token_mint: token_mint_pda,
        token_manager_pda,
        token_manager_ata,
        token_program: spl_token_2022::ID,
        gateway_root_pda,
        gateway_event_authority,
        gateway_program: axelar_solana_gateway_v2::ID,
        gas_treasury: treasury_pda,
        gas_service: axelar_solana_gas_service_v2::ID,
        gas_event_authority,
        system_program: solana_sdk::system_program::ID,
        signing_pda,
        its_program: program_id,
        event_authority: its_event_authority,
        program: program_id,
    };

    let instruction_data = axelar_solana_its_v2::instruction::InterchainTransfer {
        token_id,
        destination_chain: destination_chain.clone(),
        destination_address: destination_address.clone(),
        amount: transfer_amount,
        gas_value,
        signing_pda_bump,
        source_id: None,
        pda_seeds: None,
        data: None,
    };

    let instruction = Instruction {
        program_id,
        accounts: accounts.to_account_metas(None),
        data: instruction_data.data(),
    };

    let transfer_accounts = vec![
        (payer, result.get_account(&payer).unwrap().clone()),
        (source, result.get_account(&source).unwrap().clone()),
        (
            its_root_pda,
            result.get_account(&its_root_pda).unwrap().clone(),
        ),
        (source_ata, result.get_account(&source_ata).unwrap().clone()),
        (
            token_mint_pda,
            result.get_account(&token_mint_pda).unwrap().clone(),
        ),
        (
            token_manager_pda,
            result.get_account(&token_manager_pda).unwrap().clone(),
        ),
        (
            token_manager_ata,
            result.get_account(&token_manager_ata).unwrap().clone(),
        ),
        mollusk_svm_programs_token::token2022::keyed_account(),
        (gateway_root_pda, gateway_root_pda_account.clone()),
        (
            gateway_event_authority,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
        (
            axelar_solana_gateway_v2::ID,
            Account {
                lamports: LAMPORTS_PER_SOL,
                data: vec![],
                owner: solana_sdk::bpf_loader_upgradeable::id(),
                executable: true,
                rent_epoch: 0,
            },
        ),
        (treasury_pda, treasury_pda_account),
        (
            axelar_solana_gas_service_v2::ID,
            Account {
                lamports: LAMPORTS_PER_SOL,
                data: vec![],
                owner: solana_sdk::bpf_loader_upgradeable::id(),
                executable: true,
                rent_epoch: 0,
            },
        ),
        (
            gas_event_authority,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
        keyed_account_for_system_program(),
        (
            signing_pda,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
        (program_id, its_program_account.clone()),
        (its_event_authority, _event_authority_account),
        (program_id, its_program_account),
    ];

    let transfer_result = mollusk.process_instruction(&instruction, &transfer_accounts);

    assert!(
        transfer_result.program_result.is_ok(),
        "Interchain transfer instruction should succeed: {:?}",
        transfer_result.program_result
    );

    let source_ata_account = transfer_result.get_account(&source_ata).unwrap();

    // Parse the Token2022 account data to get the current balance
    let token_account = StateWithExtensions::<TokenAccount>::unpack(&source_ata_account.data)
        .expect("Failed to unpack source ATA data");

    let expected_balance = initial_supply - transfer_amount;
    assert_eq!(token_account.base.amount, expected_balance);
}

#[test]
fn test_interchain_transfer_lock_unlock() {
    let (setup, _, _, _, _) = setup_test_with_real_signers();

    let init_result = initialize_gateway(&setup);
    assert!(init_result.program_result.is_ok());

    let gas_service_program_id = axelar_solana_gas_service_v2::id();
    let mut mollusk = setup_mollusk(&gas_service_program_id, "axelar_solana_gas_service_v2");

    let operator = Pubkey::new_unique();
    let operator_account = Account::new(1_000_000_000, 0, &solana_sdk::system_program::ID);

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
    let mollusk = initialize_mollusk();

    let payer = Pubkey::new_unique();
    let payer_account = Account::new(10 * LAMPORTS_PER_SOL, 0, &solana_sdk::system_program::ID);

    let deployer = Pubkey::new_unique();
    let deployer_account = Account::new(10 * LAMPORTS_PER_SOL, 0, &solana_sdk::system_program::ID);

    let operator = Pubkey::new_unique();
    let operator_account = Account::new(1_000_000_000, 0, &solana_sdk::system_program::ID);

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

    let mint_keypair = Keypair::new();
    let mint_pubkey = mint_keypair.pubkey();
    let mint_authority = Keypair::new();
    let initial_supply = 1_000_000_000; // 1 billion tokens

    // Create a basic SPL token mint
    let mint_data = {
        let mut data = vec![0u8; spl_token_2022::state::Mint::LEN];
        let mint = spl_token_2022::state::Mint {
            mint_authority: Some(mint_authority.pubkey()).into(),
            supply: initial_supply,
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

    assert!(result.program_result.is_ok());

    let token_id = canonical_interchain_token_id(&mint_pubkey);
    let (token_manager_pda, _token_manager_bump) = Pubkey::find_program_address(
        &[
            axelar_solana_its_v2::seed_prefixes::TOKEN_MANAGER_SEED,
            its_root_pda.as_ref(),
            &token_id,
        ],
        &program_id,
    );

    let token_manager_ata = get_associated_token_address_with_program_id(
        &token_manager_pda,
        &mint_pubkey,
        &spl_token_2022::ID,
    );

    // Create deployer's ATA for the canonical token
    let deployer_ata =
        get_associated_token_address_with_program_id(&deployer, &mint_pubkey, &spl_token_2022::ID);

    // Create the deployer's ATA with some tokens
    let deployer_ata_data = {
        let mut data = vec![0u8; spl_token_2022::state::Account::LEN];
        let token_account = spl_token_2022::state::Account {
            mint: mint_pubkey,
            owner: deployer,
            amount: initial_supply, // Give deployer all the tokens initially
            delegate: None.into(),
            state: spl_token_2022::state::AccountState::Initialized,
            is_native: None.into(),
            delegated_amount: 0,
            close_authority: None.into(),
        };
        spl_token_2022::state::Account::pack(token_account, &mut data).unwrap();
        data
    };

    let deployer_ata_account = Account {
        lamports: Rent::default().minimum_balance(spl_token_2022::state::Account::LEN),
        data: deployer_ata_data,
        owner: spl_token_2022::ID,
        executable: false,
        rent_epoch: 0,
    };

    let source = deployer;
    let source_ata = deployer_ata;
    let destination_chain = "ethereum".to_string();
    let destination_address = b"0x1234567890123456789012345678901234567890".to_vec();
    let transfer_amount = 1_000_000u64;
    let gas_value = 0u64;

    let (signing_pda, signing_pda_bump) =
        Pubkey::find_program_address(&[CALL_CONTRACT_SIGNING_SEED], &axelar_solana_its_v2::ID);

    let (gas_event_authority, _) =
        Pubkey::find_program_address(&[b"__event_authority"], &axelar_solana_gas_service_v2::ID);

    let (gateway_event_authority, _) =
        Pubkey::find_program_address(&[b"__event_authority"], &axelar_solana_gateway_v2::ID);

    let (its_event_authority, _event_authority_account, its_program_account) =
        get_event_authority_and_program_accounts(&program_id);

    let accounts = axelar_solana_its_v2::accounts::InterchainTransfer {
        payer,
        authority: source,
        its_root_pda,
        source_ata,
        token_mint: mint_pubkey,
        token_manager_pda,
        token_manager_ata,
        token_program: spl_token_2022::ID,
        gateway_root_pda,
        gateway_event_authority,
        gateway_program: axelar_solana_gateway_v2::ID,
        gas_treasury: treasury_pda,
        gas_service: axelar_solana_gas_service_v2::ID,
        gas_event_authority,
        system_program: solana_sdk::system_program::ID,
        signing_pda,
        its_program: program_id,
        event_authority: its_event_authority,
        program: program_id,
    };

    let instruction_data = axelar_solana_its_v2::instruction::InterchainTransfer {
        token_id,
        destination_chain: destination_chain.clone(),
        destination_address: destination_address.clone(),
        amount: transfer_amount,
        gas_value,
        signing_pda_bump,
        source_id: None,
        pda_seeds: None,
        data: None,
    };

    let instruction = Instruction {
        program_id,
        accounts: accounts.to_account_metas(None),
        data: instruction_data.data(),
    };

    let transfer_accounts = vec![
        (payer, result.get_account(&payer).unwrap().clone()),
        (source, deployer_account), // Add the deployer account
        (
            its_root_pda,
            result.get_account(&its_root_pda).unwrap().clone(),
        ),
        (source_ata, deployer_ata_account), // Add the deployer's ATA
        (
            mint_pubkey, // Use the canonical mint
            result.get_account(&mint_pubkey).unwrap().clone(),
        ),
        (
            token_manager_pda,
            result.get_account(&token_manager_pda).unwrap().clone(),
        ),
        (
            token_manager_ata,
            result.get_account(&token_manager_ata).unwrap().clone(),
        ),
        mollusk_svm_programs_token::token2022::keyed_account(),
        (gateway_root_pda, gateway_root_pda_account.clone()),
        (
            gateway_event_authority,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
        (
            axelar_solana_gateway_v2::ID,
            Account {
                lamports: LAMPORTS_PER_SOL,
                data: vec![],
                owner: solana_sdk::bpf_loader_upgradeable::id(),
                executable: true,
                rent_epoch: 0,
            },
        ),
        (treasury_pda, treasury_pda_account),
        (
            axelar_solana_gas_service_v2::ID,
            Account {
                lamports: LAMPORTS_PER_SOL,
                data: vec![],
                owner: solana_sdk::bpf_loader_upgradeable::id(),
                executable: true,
                rent_epoch: 0,
            },
        ),
        (
            gas_event_authority,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
        keyed_account_for_system_program(),
        (
            signing_pda,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
        (program_id, its_program_account.clone()),
        (its_event_authority, _event_authority_account),
        (program_id, its_program_account),
    ];

    let transfer_result = mollusk.process_instruction(&instruction, &transfer_accounts);

    assert!(
        transfer_result.program_result.is_ok(),
        "Interchain transfer instruction should succeed: {:?}",
        transfer_result.program_result
    );

    let source_ata_account = transfer_result.get_account(&source_ata).unwrap();

    // Parse the Token2022 account data to get the current balance
    let token_account = StateWithExtensions::<TokenAccount>::unpack(&source_ata_account.data)
        .expect("Failed to unpack source ATA data");

    let expected_balance = initial_supply - transfer_amount;
    assert_eq!(token_account.base.amount, expected_balance);
}
