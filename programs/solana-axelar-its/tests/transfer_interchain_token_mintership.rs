#![cfg(test)]
#![allow(clippy::too_many_lines)]

use anchor_lang::{AccountDeserialize, AnchorSerialize, Discriminator};
use mollusk_svm::program::keyed_account_for_system_program;
use mollusk_svm::result::Check;
use solana_axelar_its::state::{Roles, RolesError, TokenManager, UserRoles};
use solana_axelar_its::utils::interchain_token_id;
use solana_axelar_its_test_fixtures::{
    deploy_interchain_token_helper, init_its_service, initialize_mollusk_with_programs,
    new_empty_account, new_test_account, DeployInterchainTokenContext,
};
use {
    anchor_lang::{solana_program::instruction::Instruction, InstructionData, ToAccountMetas},
    solana_sdk::pubkey::Pubkey,
};

#[test]
fn test_transfer_interchain_token_mintership_success() {
    let program_id = solana_axelar_its::id();
    let mollusk = initialize_mollusk_with_programs();

    let (upgrade_authority, _) = new_test_account();
    let payer = upgrade_authority;
    let (_, payer_account) = new_test_account();

    let (current_minter, current_minter_account) = new_test_account();
    let (new_minter, new_minter_account) = new_test_account();
    let (operator, operator_account) = new_test_account();

    let chain_name = "solana".to_owned();
    let its_hub_address = "0x123456789abcdef".to_owned();

    let (its_root_pda, its_root_account, _, _, _, _) = init_its_service(
        &mollusk,
        payer,
        &payer_account,
        upgrade_authority,
        operator,
        &operator_account,
        chain_name.clone(),
        its_hub_address.clone(),
    );

    // Deploy an interchain token to create a TokenManager PDA
    let salt = [1u8; 32];
    let token_name = "Test Token".to_owned();
    let token_symbol = "TEST".to_owned();
    let decimals = 9u8;
    let initial_supply = 1_000_000_000u64;

    // Calculate the token manager and minter roles PDAs
    let token_id = interchain_token_id(&current_minter, &salt);
    let (token_manager_pda, _) = TokenManager::find_pda(token_id, its_root_pda);
    let (current_minter_roles_pda, _) = UserRoles::find_pda(&token_manager_pda, &current_minter);

    let ctx = DeployInterchainTokenContext::new(
        mollusk,
        (its_root_pda, its_root_account.clone()),
        (current_minter, current_minter_account.clone()),
        (payer, payer_account.clone()),
        Some(current_minter), // minter (will get MINTER role for token manager)
        Some(current_minter_roles_pda), // minter_roles_pda
    );

    let (deploy_result, token_manager_pda, _, _, _, _, mollusk) = deploy_interchain_token_helper(
        ctx,
        salt,
        token_name,
        token_symbol,
        decimals,
        initial_supply,
        vec![Check::success()],
    );

    assert!(deploy_result.program_result.is_ok());

    let token_manager_account = deploy_result
        .get_account(&token_manager_pda)
        .expect("TokenManager account should exist");

    let token_manager_data =
        TokenManager::try_deserialize(&mut token_manager_account.data.as_slice())
            .expect("Failed to deserialize TokenManager");
    assert_eq!(token_manager_data.token_id, token_id);

    let current_minter_token_roles_account = deploy_result
        .get_account(&current_minter_roles_pda)
        .expect("Current minter token roles account should exist");

    let current_minter_token_roles =
        UserRoles::try_deserialize(&mut current_minter_token_roles_account.data.as_slice())
            .expect("Failed to deserialize current minter token roles");
    assert!(
        current_minter_token_roles.roles.contains(Roles::MINTER),
        "Current minter should have MINTER role for token manager"
    );

    let (new_minter_token_roles_pda, new_minter_token_roles_pda_bump) =
        UserRoles::find_pda(&token_manager_pda, &new_minter);

    let ix = Instruction {
        program_id,
        accounts: solana_axelar_its::accounts::TransferInterchainTokenMintership {
            its_root_pda,
            system_program: solana_sdk::system_program::ID,
            payer,
            sender_user_account: current_minter,
            sender_roles_account: current_minter_roles_pda,
            token_manager_account: token_manager_pda,
            destination_user_account: new_minter,
            destination_roles_account: new_minter_token_roles_pda,
        }
        .to_account_metas(None),
        data: solana_axelar_its::instruction::TransferInterchainTokenMintership {}.data(),
    };

    let accounts = vec![
        (its_root_pda, its_root_account.clone()),
        keyed_account_for_system_program(),
        (payer, payer_account.clone()),
        (current_minter, current_minter_account.clone()),
        (
            current_minter_roles_pda,
            current_minter_token_roles_account.clone(),
        ),
        (token_manager_pda, token_manager_account.clone()),
        (new_minter, new_minter_account.clone()),
        (new_minter_token_roles_pda, new_empty_account()),
    ];

    let result = mollusk.process_instruction(&ix, &accounts);

    assert!(result.program_result.is_ok());

    // Verify that the old minter no longer has MINTER role
    let old_minter_token_roles_account = result
        .get_account(&current_minter_roles_pda)
        .expect("Current minter token roles account should exist");

    let old_minter_token_roles =
        UserRoles::try_deserialize(&mut old_minter_token_roles_account.data.as_slice())
            .expect("Failed to deserialize current minter token roles");
    assert!(
        !old_minter_token_roles.roles.contains(Roles::MINTER),
        "Old minter should not have MINTER role for token manager"
    );

    // Verify that the new minter has MINTER role
    let new_minter_token_roles_account = result
        .get_account(&new_minter_token_roles_pda)
        .expect("New minter token roles account should exist");

    let new_minter_token_roles =
        UserRoles::try_deserialize(&mut new_minter_token_roles_account.data.as_slice())
            .expect("Failed to deserialize new minter token roles");

    assert_eq!(new_minter_token_roles.bump, new_minter_token_roles_pda_bump);

    assert!(
        new_minter_token_roles.roles.contains(Roles::MINTER),
        "New minter should have MINTER role for token manager"
    );
}

#[test]
fn test_reject_transfer_interchain_token_mintership_with_unauthorized_minter() {
    let program_id = solana_axelar_its::id();
    let mollusk = initialize_mollusk_with_programs();

    let (upgrade_authority, _) = new_test_account();
    let payer = upgrade_authority;
    let (_, payer_account) = new_test_account();

    let (current_minter, current_minter_account) = new_test_account();
    let (new_minter, new_minter_account) = new_test_account();
    let (operator, operator_account) = new_test_account();

    let chain_name = "solana".to_owned();
    let its_hub_address = "0x123456789abcdef".to_owned();

    let (its_root_pda, its_root_account, _, _, _, _) = init_its_service(
        &mollusk,
        payer,
        &payer_account,
        upgrade_authority,
        operator,
        &operator_account,
        chain_name.clone(),
        its_hub_address.clone(),
    );

    // Deploy an interchain token to create a TokenManager PDA
    let salt = [1u8; 32];
    let token_name = "Test Token".to_owned();
    let token_symbol = "TEST".to_owned();
    let decimals = 9u8;
    let initial_supply = 1_000_000_000u64;

    // Calculate the token manager and minter roles PDAs
    let token_id = interchain_token_id(&current_minter, &salt);
    let (token_manager_pda, _) = TokenManager::find_pda(token_id, its_root_pda);
    let (current_minter_roles_pda, _) = UserRoles::find_pda(&token_manager_pda, &current_minter);

    let ctx = DeployInterchainTokenContext::new(
        mollusk,
        (its_root_pda, its_root_account.clone()),
        (current_minter, current_minter_account.clone()),
        (payer, payer_account.clone()),
        Some(current_minter), // minter (will get MINTER role for token manager)
        Some(current_minter_roles_pda), // minter_roles_pda
    );

    let (deploy_result, token_manager_pda, _, _, _, _, mollusk) = deploy_interchain_token_helper(
        ctx,
        salt,
        token_name,
        token_symbol,
        decimals,
        initial_supply,
        vec![Check::success()],
    );

    assert!(deploy_result.program_result.is_ok());

    let token_manager_account = deploy_result
        .get_account(&token_manager_pda)
        .expect("TokenManager account should exist");

    let current_minter_token_roles_account = deploy_result
        .get_account(&current_minter_roles_pda)
        .expect("Current minter token roles account should exist");

    let current_minter_token_roles =
        UserRoles::try_deserialize(&mut current_minter_token_roles_account.data.as_slice())
            .expect("Failed to deserialize current minter token roles");
    assert!(
        current_minter_token_roles.roles.contains(Roles::MINTER),
        "Current minter should have MINTER role for token manager"
    );

    let (new_minter_token_roles_pda, _) = UserRoles::find_pda(&token_manager_pda, &new_minter);

    let malicious_minter = Pubkey::new_unique();

    let ix = Instruction {
        program_id,
        accounts: solana_axelar_its::accounts::TransferInterchainTokenMintership {
            its_root_pda,
            system_program: solana_sdk::system_program::ID,
            payer,
            sender_user_account: malicious_minter,
            sender_roles_account: current_minter_roles_pda,
            token_manager_account: token_manager_pda,
            destination_user_account: new_minter,
            destination_roles_account: new_minter_token_roles_pda,
        }
        .to_account_metas(None),
        data: solana_axelar_its::instruction::TransferInterchainTokenMintership {}.data(),
    };

    let accounts = vec![
        (its_root_pda, its_root_account.clone()),
        keyed_account_for_system_program(),
        (payer, payer_account.clone()),
        (malicious_minter, current_minter_account.clone()),
        (
            current_minter_roles_pda,
            current_minter_token_roles_account.clone(),
        ),
        (token_manager_pda, token_manager_account.clone()),
        (new_minter, new_minter_account.clone()),
        (new_minter_token_roles_pda, new_empty_account()),
    ];

    let checks = vec![Check::err(
        anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::ConstraintSeeds).into(),
    )];

    mollusk.process_and_validate_instruction(&ix, &accounts, &checks);
}

#[test]
fn test_reject_transfer_interchain_token_mintership_without_minter_role() {
    let program_id = solana_axelar_its::id();
    let mollusk = initialize_mollusk_with_programs();

    let (upgrade_authority, _) = new_test_account();
    let payer = upgrade_authority;
    let (_, payer_account) = new_test_account();

    let (current_minter, current_minter_account) = new_test_account();
    let (new_minter, new_minter_account) = new_test_account();
    let (operator, operator_account) = new_test_account();

    let chain_name = "solana".to_owned();
    let its_hub_address = "0x123456789abcdef".to_owned();

    let (its_root_pda, its_root_account, _, _, _, _) = init_its_service(
        &mollusk,
        payer,
        &payer_account,
        upgrade_authority,
        operator,
        &operator_account,
        chain_name.clone(),
        its_hub_address.clone(),
    );

    // Deploy an interchain token to create a TokenManager PDA
    let salt = [1u8; 32];
    let token_name = "Test Token".to_owned();
    let token_symbol = "TEST".to_owned();
    let decimals = 9u8;
    let initial_supply = 1_000_000_000u64;

    // Calculate the token manager and minter roles PDAs
    let token_id = interchain_token_id(&current_minter, &salt);
    let (token_manager_pda, _) = TokenManager::find_pda(token_id, its_root_pda);
    let (current_minter_roles_pda, _) = UserRoles::find_pda(&token_manager_pda, &current_minter);

    let ctx = DeployInterchainTokenContext::new(
        mollusk,
        (its_root_pda, its_root_account.clone()),
        (current_minter, current_minter_account.clone()),
        (payer, payer_account.clone()),
        Some(current_minter), // minter (will get MINTER role for token manager)
        Some(current_minter_roles_pda), // minter_roles_pda
    );

    let (deploy_result, token_manager_pda, _, _, _, _, mollusk) = deploy_interchain_token_helper(
        ctx,
        salt,
        token_name,
        token_symbol,
        decimals,
        initial_supply,
        vec![Check::success()],
    );

    assert!(deploy_result.program_result.is_ok());

    let token_manager_account = deploy_result
        .get_account(&token_manager_pda)
        .expect("TokenManager account should exist");

    let current_minter_token_roles_account = deploy_result
        .get_account(&current_minter_roles_pda)
        .expect("Current minter token roles account should exist");

    // Remove MINTER role from current minter account
    let mut current_minter_token_roles_account_clone = current_minter_token_roles_account.clone();

    let mut current_minter_token_roles =
        UserRoles::try_deserialize(&mut current_minter_token_roles_account_clone.data.as_ref())
            .expect("Failed to deserialize minter roles");
    current_minter_token_roles.roles = Roles::empty();

    let mut new_data = Vec::new();
    new_data.extend_from_slice(UserRoles::DISCRIMINATOR);
    current_minter_token_roles
        .serialize(&mut new_data)
        .expect("Failed to serialize");
    current_minter_token_roles_account_clone.data = new_data;

    let (new_minter_token_roles_pda, _) = UserRoles::find_pda(&token_manager_pda, &new_minter);

    let ix = Instruction {
        program_id,
        accounts: solana_axelar_its::accounts::TransferInterchainTokenMintership {
            its_root_pda,
            system_program: solana_sdk::system_program::ID,
            payer,
            sender_user_account: current_minter,
            sender_roles_account: current_minter_roles_pda,
            token_manager_account: token_manager_pda,
            destination_user_account: new_minter,
            destination_roles_account: new_minter_token_roles_pda,
        }
        .to_account_metas(None),
        data: solana_axelar_its::instruction::TransferInterchainTokenMintership {}.data(),
    };

    let accounts = vec![
        (its_root_pda, its_root_account.clone()),
        keyed_account_for_system_program(),
        (payer, payer_account.clone()),
        (current_minter, current_minter_account.clone()),
        (
            current_minter_roles_pda,
            current_minter_token_roles_account_clone,
        ),
        (token_manager_pda, token_manager_account.clone()),
        (new_minter, new_minter_account.clone()),
        (new_minter_token_roles_pda, new_empty_account()),
    ];

    let checks = vec![Check::err(
        anchor_lang::error::Error::from(RolesError::MissingMinterRole).into(),
    )];

    mollusk.process_and_validate_instruction(&ix, &accounts, &checks);
}

#[test]
fn test_reject_transfer_interchain_token_mintership_same_sender_destination() {
    let program_id = solana_axelar_its::id();
    let mollusk = initialize_mollusk_with_programs();

    let (upgrade_authority, _) = new_test_account();
    let payer = upgrade_authority;
    let (_, payer_account) = new_test_account();

    let (current_minter, current_minter_account) = new_test_account();
    let (operator, operator_account) = new_test_account();

    let chain_name = "solana".to_owned();
    let its_hub_address = "0x123456789abcdef".to_owned();

    let (its_root_pda, its_root_account, _, _, _, _) = init_its_service(
        &mollusk,
        payer,
        &payer_account,
        upgrade_authority,
        operator,
        &operator_account,
        chain_name.clone(),
        its_hub_address.clone(),
    );

    // Deploy an interchain token to create a TokenManager PDA
    let salt = [1u8; 32];
    let token_name = "Test Token".to_owned();
    let token_symbol = "TEST".to_owned();
    let decimals = 9u8;
    let initial_supply = 1_000_000_000u64;

    // Calculate the token manager and minter roles PDAs
    let token_id = interchain_token_id(&current_minter, &salt);
    let (token_manager_pda, _) = TokenManager::find_pda(token_id, its_root_pda);
    let (current_minter_roles_pda, _) = UserRoles::find_pda(&token_manager_pda, &current_minter);

    let ctx = DeployInterchainTokenContext::new(
        mollusk,
        (its_root_pda, its_root_account.clone()),
        (current_minter, current_minter_account.clone()),
        (payer, payer_account.clone()),
        Some(current_minter), // minter (will get MINTER role for token manager)
        Some(current_minter_roles_pda), // minter_roles_pda
    );

    let (deploy_result, token_manager_pda, _, _, _, _, mollusk) = deploy_interchain_token_helper(
        ctx,
        salt,
        token_name,
        token_symbol,
        decimals,
        initial_supply,
        vec![Check::success()],
    );

    assert!(deploy_result.program_result.is_ok());

    let token_manager_account = deploy_result
        .get_account(&token_manager_pda)
        .expect("TokenManager account should exist");

    let current_minter_token_roles_account = deploy_result
        .get_account(&current_minter_roles_pda)
        .expect("Current minter token roles account should exist");

    let ix = Instruction {
        program_id,
        accounts: solana_axelar_its::accounts::TransferInterchainTokenMintership {
            its_root_pda,
            system_program: solana_sdk::system_program::ID,
            payer,
            sender_user_account: current_minter,
            sender_roles_account: current_minter_roles_pda,
            token_manager_account: token_manager_pda,
            destination_user_account: current_minter, // Same as sender
            destination_roles_account: current_minter_roles_pda, // Same as sender
        }
        .to_account_metas(None),
        data: solana_axelar_its::instruction::TransferInterchainTokenMintership {}.data(),
    };

    let accounts = vec![
        (its_root_pda, its_root_account.clone()),
        keyed_account_for_system_program(),
        (payer, payer_account.clone()),
        (current_minter, current_minter_account.clone()),
        (
            current_minter_roles_pda,
            current_minter_token_roles_account.clone(),
        ),
        (token_manager_pda, token_manager_account.clone()),
        (current_minter, current_minter_account.clone()),
        (
            current_minter_roles_pda,
            current_minter_token_roles_account.clone(),
        ),
    ];

    let checks = vec![Check::err(
        anchor_lang::error::Error::from(solana_axelar_its::ItsError::InvalidArgument).into(),
    )];

    mollusk.process_and_validate_instruction(&ix, &accounts, &checks);
}
