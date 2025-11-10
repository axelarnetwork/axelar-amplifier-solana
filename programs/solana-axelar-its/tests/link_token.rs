#![cfg(test)]
#![allow(clippy::too_many_lines)]

use anchor_lang::AccountDeserialize;
use anchor_lang::{InstructionData, ToAccountMetas};
use anchor_spl::token_2022::spl_token_2022;
use mollusk_svm::program::keyed_account_for_system_program;
use mollusk_test_utils::get_event_authority_and_program_accounts;
use mollusk_test_utils::setup_mollusk;
use solana_axelar_gateway::seed_prefixes::GATEWAY_SEED;
use solana_axelar_gateway::ID as GATEWAY_PROGRAM_ID;
use solana_axelar_gateway_test_fixtures::initialize_gateway;
use solana_axelar_gateway_test_fixtures::setup_test_with_real_signers;
use solana_axelar_its::utils::linked_token_deployer_salt;
use solana_axelar_its::{
    state::{token_manager::Type, TokenManager},
    utils::interchain_token_id_internal,
};
use solana_axelar_its_test_fixtures::create_test_mint;
use solana_axelar_its_test_fixtures::init_gas_service;
use solana_axelar_its_test_fixtures::init_its_service_with_ethereum_trusted;
use solana_axelar_its_test_fixtures::initialize_mollusk;
use solana_axelar_its_test_fixtures::setup_operator;
use solana_sdk::{
    account::Account, instruction::Instruction, native_token::LAMPORTS_PER_SOL, pubkey::Pubkey,
};

#[test]
fn test_link_token() {
    let (setup, _, _, _, _) = setup_test_with_real_signers();
    let init_result = initialize_gateway(&setup);
    assert!(init_result.program_result.is_ok());

    let gas_service_program_id = solana_axelar_gas_service::id();
    let mut gas_service_mollusk =
        setup_mollusk(&gas_service_program_id, "solana_axelar_gas_service");

    let gas_operator = Pubkey::new_unique();
    let gas_operator_account = Account::new(1_000_000_000, 0, &solana_sdk::system_program::ID);

    let (gas_operator_pda, gas_operator_pda_account) = setup_operator(
        &mut gas_service_mollusk,
        gas_operator,
        &gas_operator_account,
    );

    // Use the GAS SERVICE mollusk for gas service initialization
    let (_, treasury_account) = init_gas_service(
        &gas_service_mollusk,
        gas_operator,
        &gas_operator_account,
        gas_operator_pda,
        &gas_operator_pda_account,
    );

    let (gateway_root_pda, _) = Pubkey::find_program_address(&[GATEWAY_SEED], &GATEWAY_PROGRAM_ID);
    let gateway_root_pda_account = init_result.get_account(&gateway_root_pda).unwrap();

    let program_id = solana_axelar_its::id();
    let mollusk = initialize_mollusk();

    let payer = Pubkey::new_unique();
    let payer_account = Account::new(10 * LAMPORTS_PER_SOL, 0, &solana_sdk::system_program::ID);

    let deployer = Pubkey::new_unique();
    let deployer_account = Account::new(10 * LAMPORTS_PER_SOL, 0, &solana_sdk::system_program::ID);

    let its_operator = Pubkey::new_unique();
    let its_operator_account = Account::new(1_000_000_000, 0, &solana_sdk::system_program::ID);

    let chain_name = "solana".to_owned();
    let its_hub_address = "0x123456789abcdef".to_owned();

    let (its_root_pda, its_root_account) = init_its_service_with_ethereum_trusted(
        &mollusk,
        payer,
        &payer_account,
        payer,
        its_operator,
        &its_operator_account,
        chain_name.clone(),
        its_hub_address.clone(),
    );

    // Create a test mint (existing token to register)
    let mint_authority = Pubkey::new_unique();
    let (token_mint, token_mint_account) = create_test_mint(mint_authority);

    // Register custom token parameters
    let salt = [2u8; 32];
    let token_manager_type = Type::LockUnlock; // Use LockUnlock, NOT NativeInterchainToken
    let operator_param: Option<Pubkey> = None; // No operator

    let token_id = {
        let deploy_salt = linked_token_deployer_salt(&deployer, &salt);
        interchain_token_id_internal(&deploy_salt)
    };

    let (token_manager_pda, _token_manager_bump) = Pubkey::find_program_address(
        &[
            solana_axelar_its::seed_prefixes::TOKEN_MANAGER_SEED,
            its_root_pda.as_ref(),
            &token_id,
        ],
        &program_id,
    );

    let token_manager_ata =
        anchor_spl::associated_token::get_associated_token_address_with_program_id(
            &token_manager_pda,
            &token_mint,
            &spl_token_2022::ID,
        );

    // Create the register custom token instruction first
    let register_instruction_data = solana_axelar_its::instruction::RegisterCustomToken {
        salt,
        token_manager_type,
        operator: operator_param,
    };

    let (event_authority, _, _) = get_event_authority_and_program_accounts(&program_id);

    // Build account metas for register custom token
    let register_accounts = solana_axelar_its::accounts::RegisterCustomToken {
        payer,
        deployer,
        system_program: solana_sdk::system_program::ID,
        its_root_pda,
        token_manager_pda,
        token_mint,
        token_manager_ata,
        token_program: spl_token_2022::ID,
        associated_token_program: anchor_spl::associated_token::ID,
        operator: None,
        operator_roles_pda: None,
        // for event cpi
        event_authority,
        program: program_id,
    };

    let register_ix = Instruction {
        program_id,
        accounts: register_accounts.to_account_metas(None),
        data: register_instruction_data.data(),
    };

    // Set up accounts for register custom token
    let register_mollusk_accounts = vec![
        (payer, payer_account.clone()),
        (deployer, deployer_account.clone()),
        keyed_account_for_system_program(),
        (its_root_pda, its_root_account.clone()),
        (
            token_manager_pda,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
        (token_mint, token_mint_account),
        (
            token_manager_ata,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
        mollusk_svm_programs_token::token2022::keyed_account(),
        mollusk_svm_programs_token::associated_token::keyed_account(),
        (
            anchor_spl::associated_token::ID,
            Account {
                lamports: 1,
                data: vec![],
                owner: solana_sdk::bpf_loader_upgradeable::ID,
                executable: true,
                rent_epoch: 0,
            },
        ),
        (
            solana_sdk::sysvar::rent::ID,
            Account {
                lamports: 1_000_000_000,
                data: {
                    let rent = anchor_lang::prelude::Rent::default();
                    bincode::serialize(&rent).unwrap()
                },
                owner: solana_sdk::sysvar::rent::ID,
                executable: false,
                rent_epoch: 0,
            },
        ),
        // For event CPI
        (
            event_authority,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
        (
            program_id,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
    ];

    let register_result = mollusk.process_and_validate_instruction(
        &register_ix,
        &register_mollusk_accounts,
        &[mollusk_svm::result::Check::success()],
    );

    assert!(
        register_result.program_result.is_ok(),
        "Register custom token instruction should succeed: {:?}",
        register_result.program_result
    );

    // Get the updated token manager account after registration
    let token_manager_account = register_result.get_account(&token_manager_pda).unwrap();

    // Verify token manager was created correctly
    let token_manager =
        TokenManager::try_deserialize(&mut token_manager_account.data.as_ref()).unwrap();
    assert_eq!(token_manager.ty, Type::LockUnlock);

    // Link token test parameters
    let destination_chain = "ethereum".to_owned();
    let destination_token_address = vec![0x12, 0x34, 0x56, 0x78]; // Mock Ethereum address
    let link_params = vec![]; // No additional params
    let gas_value = 0u64; // No gas payment for this test

    // Derive required PDAs
    let (gas_treasury, _) = Pubkey::find_program_address(
        &[solana_axelar_gas_service::state::Treasury::SEED_PREFIX],
        &solana_axelar_gas_service::ID,
    );

    let (call_contract_signing_pda, _signing_pda_bump) = Pubkey::find_program_address(
        &[solana_axelar_gateway::seed_prefixes::CALL_CONTRACT_SIGNING_SEED],
        &program_id,
    );

    let (gateway_event_authority, _, _) =
        get_event_authority_and_program_accounts(&solana_axelar_gateway::ID);

    let (gas_event_authority, _, _) =
        get_event_authority_and_program_accounts(&solana_axelar_gas_service::ID);

    // Create link token instruction
    let link_instruction_data = solana_axelar_its::instruction::LinkToken {
        salt,
        destination_chain: destination_chain.clone(),
        destination_token_address: destination_token_address.clone(),
        token_manager_type,
        link_params: link_params.clone(),
        gas_value,
    };

    // Build accounts
    let link_accounts = solana_axelar_its::accounts::LinkToken {
        payer,
        deployer,
        its_root_pda,
        token_manager_pda,
        gateway_root_pda,
        gateway_program: solana_axelar_gateway::ID,
        system_program: solana_sdk::system_program::ID,
        call_contract_signing_pda,
        gateway_event_authority,
        gas_service_accounts: solana_axelar_its::accounts::GasServiceAccounts {
            gas_treasury,
            gas_service: solana_axelar_gas_service::ID,
            gas_event_authority,
        },
        // for event cpi
        event_authority,
        program: program_id,
    };

    let link_ix = Instruction {
        program_id,
        accounts: link_accounts.to_account_metas(None),
        data: link_instruction_data.data(),
    };

    // Setup accounts for mollusk
    let link_mollusk_accounts = vec![
        (payer, payer_account),
        (deployer, deployer_account),
        (token_manager_pda, token_manager_account.clone()),
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
        (gas_treasury, treasury_account),
        (
            solana_axelar_gas_service::ID,
            Account {
                lamports: 1,
                data: vec![],
                owner: solana_sdk::bpf_loader_upgradeable::ID,
                executable: true,
                rent_epoch: 0,
            },
        ),
        keyed_account_for_system_program(),
        (its_root_pda, its_root_account),
        (
            call_contract_signing_pda,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
        (
            program_id,
            Account::new(0, 0, &solana_sdk::system_program::ID),
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
        (
            event_authority,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
        (
            program_id,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
    ];

    let link_result = mollusk.process_and_validate_instruction(
        &link_ix,
        &link_mollusk_accounts,
        &[mollusk_svm::result::Check::success()],
    );

    assert!(
        link_result.program_result.is_ok(),
        "Link token instruction should succeed: {:?}",
        link_result.program_result
    );
}
