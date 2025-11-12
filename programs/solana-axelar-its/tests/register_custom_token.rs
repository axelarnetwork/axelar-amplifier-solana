#![cfg(test)]
#![allow(clippy::too_many_lines)]

use anchor_lang::AccountDeserialize;
use anchor_lang::{InstructionData, ToAccountMetas};
use anchor_spl::token_2022::spl_token_2022;
use mollusk_svm::program::keyed_account_for_system_program;
use mollusk_svm::result::Check;
use mollusk_test_utils::get_event_authority_and_program_accounts;
use solana_axelar_its::state::UserRoles;
use solana_axelar_its::ItsError;
use solana_axelar_its::{
    state::{token_manager::Type, TokenManager},
    utils::{interchain_token_id_internal, linked_token_deployer_salt},
};
use solana_axelar_its_test_fixtures::{
    create_test_mint, execute_register_custom_token_helper, RegisterCustomTokenContext,
    RegisterCustomTokenParams,
};
use solana_axelar_its_test_fixtures::{init_its_service, initialize_mollusk};
use solana_sdk::{
    account::Account, instruction::Instruction, native_token::LAMPORTS_PER_SOL, pubkey::Pubkey,
};

#[test]
fn test_register_custom_token_without_operator() {
    let mollusk = initialize_mollusk();

    let payer = Pubkey::new_unique();
    let payer_account = Account::new(10 * LAMPORTS_PER_SOL, 0, &solana_sdk::system_program::ID);

    let deployer = Pubkey::new_unique();
    let deployer_account = Account::new(10 * LAMPORTS_PER_SOL, 0, &solana_sdk::system_program::ID);

    let operator = Pubkey::new_unique();
    let operator_account = Account::new(1_000_000_000, 0, &solana_sdk::system_program::ID);

    let chain_name = "solana".to_owned();
    let its_hub_address = "0x123456789abcdef".to_owned();

    // Initialize ITS service first
    let (its_root_pda, its_root_account, _, _, _, _) = init_its_service(
        &mollusk,
        payer,
        &payer_account,
        payer,
        operator,
        &operator_account,
        chain_name.clone(),
        its_hub_address.clone(),
    );

    // Create a test mint (existing token to register)
    let mint_authority = Pubkey::new_unique();
    let (token_mint, token_mint_account) = create_test_mint(mint_authority);

    // Register custom token parameters
    let salt = [2u8; 32];
    let token_manager_type = Type::LockUnlock;

    let ctx = RegisterCustomTokenContext {
        mollusk,
        payer: (payer, payer_account.clone()),
        deployer: (deployer, deployer_account.clone()),
        its_root: (its_root_pda, its_root_account.clone()),
        token_mint: (token_mint, token_mint_account),
    };

    let params = RegisterCustomTokenParams {
        salt,
        token_manager_type,
        operator: None, // No operator
    };

    let result = execute_register_custom_token_helper(ctx, params, vec![Check::success()]);

    assert!(
        result.result.program_result.is_ok(),
        "Register custom token instruction should succeed: {:?}",
        result.result.program_result
    );

    // Verify token manager was created correctly
    let token_manager_account = result
        .result
        .get_account(&result.token_manager_pda)
        .unwrap();
    let token_manager =
        TokenManager::try_deserialize(&mut token_manager_account.data.as_ref()).unwrap();

    let expected_token_id = {
        let deploy_salt = linked_token_deployer_salt(&deployer, &salt);
        interchain_token_id_internal(&deploy_salt)
    };

    assert_eq!(token_manager.ty, Type::LockUnlock);
    assert_eq!(token_manager.token_id, expected_token_id);
    assert_eq!(token_manager.token_address, token_mint);
    assert_eq!(
        token_manager.associated_token_account,
        result.token_manager_ata
    );
    assert_eq!(token_manager.flow_slot.flow_limit, None);
}

#[test]
fn test_reject_register_custom_token_with_native_interchain() {
    let mollusk = initialize_mollusk();

    let payer = Pubkey::new_unique();
    let payer_account = Account::new(10 * LAMPORTS_PER_SOL, 0, &solana_sdk::system_program::ID);

    let deployer = Pubkey::new_unique();
    let deployer_account = Account::new(10 * LAMPORTS_PER_SOL, 0, &solana_sdk::system_program::ID);

    let operator = Pubkey::new_unique();
    let operator_account = Account::new(1_000_000_000, 0, &solana_sdk::system_program::ID);

    let chain_name = "solana".to_owned();
    let its_hub_address = "0x123456789abcdef".to_owned();

    // Initialize ITS service first
    let (its_root_pda, its_root_account, _, _, _, _) = init_its_service(
        &mollusk,
        payer,
        &payer_account,
        payer,
        operator,
        &operator_account,
        chain_name.clone(),
        its_hub_address.clone(),
    );

    // Create a test mint (existing token to register)
    let mint_authority = Pubkey::new_unique();
    let (token_mint, token_mint_account) = create_test_mint(mint_authority);

    // Register custom token parameters
    let salt = [2u8; 32];
    let token_manager_type = Type::NativeInterchainToken; // not allowed for custom tokens

    let ctx = RegisterCustomTokenContext {
        mollusk,
        payer: (payer, payer_account.clone()),
        deployer: (deployer, deployer_account.clone()),
        its_root: (its_root_pda, its_root_account.clone()),
        token_mint: (token_mint, token_mint_account),
    };

    let params = RegisterCustomTokenParams {
        salt,
        token_manager_type,
        operator: None, // No operator
    };

    let checks = vec![Check::err(
        anchor_lang::error::Error::from(ItsError::InvalidInstructionData).into(),
    )];

    let result = execute_register_custom_token_helper(ctx, params, checks);
    assert!(result.result.program_result.is_err());
}

#[test]
fn test_register_custom_token_with_operator() {
    let mollusk = initialize_mollusk();

    let payer = Pubkey::new_unique();
    let payer_account = Account::new(10 * LAMPORTS_PER_SOL, 0, &solana_sdk::system_program::ID);

    let deployer = Pubkey::new_unique();
    let deployer_account = Account::new(10 * LAMPORTS_PER_SOL, 0, &solana_sdk::system_program::ID);

    let operator = Pubkey::new_unique();
    let operator_account = Account::new(1_000_000_000, 0, &solana_sdk::system_program::ID);

    let chain_name = "solana".to_owned();
    let its_hub_address = "0x123456789abcdef".to_owned();

    // Initialize ITS service first
    let (its_root_pda, its_root_account, _, _, _, _) = init_its_service(
        &mollusk,
        payer,
        &payer_account,
        payer,
        operator,
        &operator_account,
        chain_name.clone(),
        its_hub_address.clone(),
    );

    // Create a test mint (existing token to register)
    let mint_authority = Pubkey::new_unique();
    let (token_mint, token_mint_account) = create_test_mint(mint_authority);

    // Register custom token parameters
    let salt = [2u8; 32];
    let token_manager_type = Type::LockUnlock;
    let operator = Pubkey::new_unique();

    let ctx = RegisterCustomTokenContext {
        mollusk,
        payer: (payer, payer_account.clone()),
        deployer: (deployer, deployer_account.clone()),
        its_root: (its_root_pda, its_root_account.clone()),
        token_mint: (token_mint, token_mint_account),
    };

    let params = RegisterCustomTokenParams {
        salt,
        token_manager_type,
        operator: Some(operator),
    };

    let result = execute_register_custom_token_helper(ctx, params, vec![Check::success()]);

    assert!(result.result.program_result.is_ok());

    // Verify token manager was created correctly
    let token_manager_account = result
        .result
        .get_account(&result.token_manager_pda)
        .unwrap();
    let token_manager =
        TokenManager::try_deserialize(&mut token_manager_account.data.as_ref()).unwrap();

    let expected_token_id = {
        let deploy_salt = linked_token_deployer_salt(&deployer, &salt);
        interchain_token_id_internal(&deploy_salt)
    };

    assert_eq!(token_manager.ty, Type::LockUnlock);
    assert_eq!(token_manager.token_id, expected_token_id);
    assert_eq!(token_manager.token_address, token_mint);
    assert_eq!(
        token_manager.associated_token_account,
        result.token_manager_ata
    );
    assert_eq!(token_manager.flow_slot.flow_limit, None);
}

#[test]
fn test_reject_register_custom_token_with_mismatched_operator() {
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

    // Initialize ITS service first
    let (its_root_pda, its_root_account, _, _, _, _) = init_its_service(
        &mollusk,
        payer,
        &payer_account,
        payer,
        operator,
        &operator_account,
        chain_name.clone(),
        its_hub_address.clone(),
    );

    // Create a test mint (existing token to register)
    let mint_authority = Pubkey::new_unique();
    let (token_mint, token_mint_account) = create_test_mint(mint_authority);

    // Register custom token parameters
    let salt = [2u8; 32];
    let token_manager_type = Type::LockUnlock;

    let token_id = {
        let deploy_salt = linked_token_deployer_salt(&deployer, &salt);
        interchain_token_id_internal(&deploy_salt)
    };

    let (token_manager_pda, _) = TokenManager::find_pda(token_id, its_root_pda);

    let token_manager_ata =
        anchor_spl::associated_token::get_associated_token_address_with_program_id(
            &token_manager_pda,
            &token_mint,
            &spl_token_2022::ID,
        );

    let (event_authority, _, _) = get_event_authority_and_program_accounts(&program_id);

    let operator = Pubkey::new_unique();
    let (operator_roles_pda, _) = UserRoles::find_pda(&token_manager_pda, &operator);

    // Create the instruction data
    let instruction_data = solana_axelar_its::instruction::RegisterCustomToken {
        salt,
        token_manager_type,
        operator: None,
    };

    // Build account metas
    let accounts = solana_axelar_its::accounts::RegisterCustomToken {
        payer,
        deployer,
        system_program: solana_sdk::system_program::ID,
        its_root_pda,
        token_manager_pda,
        token_mint,
        token_manager_ata,
        token_program: spl_token_2022::ID,
        associated_token_program: anchor_spl::associated_token::ID,
        operator: Some(operator),
        operator_roles_pda: Some(operator_roles_pda),
        // for event cpi
        event_authority,
        program: program_id,
    };

    let ix = Instruction {
        program_id,
        accounts: accounts.to_account_metas(None),
        data: instruction_data.data(),
    };

    // Set up accounts for mollusk
    let mollusk_accounts = vec![
        (payer, payer_account),
        (deployer, deployer_account),
        keyed_account_for_system_program(),
        (its_root_pda, its_root_account),
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
            operator,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
        (
            operator_roles_pda,
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

    let checks = vec![Check::err(
        anchor_lang::error::Error::from(ItsError::InvalidArgument).into(),
    )];

    let result = mollusk.process_and_validate_instruction(&ix, &mollusk_accounts, &checks);

    assert!(result.program_result.is_err());
}
