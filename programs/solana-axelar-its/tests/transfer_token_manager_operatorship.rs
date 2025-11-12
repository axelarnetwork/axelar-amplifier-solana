#![cfg(test)]
#![allow(clippy::too_many_lines)]

use anchor_lang::{AccountDeserialize, AnchorSerialize, Discriminator};
use mollusk_svm::program::keyed_account_for_system_program;
use mollusk_svm::result::Check;
use solana_axelar_its::state::{Roles, RolesError, TokenManager, UserRoles};
use solana_axelar_its::utils::interchain_token_id;
use solana_axelar_its_test_fixtures::{
    deploy_interchain_token_helper, init_its_service, initialize_mollusk,
    DeployInterchainTokenContext,
};
use {
    anchor_lang::{solana_program::instruction::Instruction, InstructionData, ToAccountMetas},
    solana_sdk::{account::Account, native_token::LAMPORTS_PER_SOL, pubkey::Pubkey},
};

#[test]
fn test_transfer_token_manager_operatorship_success() {
    let program_id = solana_axelar_its::id();
    let mollusk = initialize_mollusk();

    let upgrade_authority = Pubkey::new_unique();
    let payer = upgrade_authority;
    let payer_account = Account::new(10 * LAMPORTS_PER_SOL, 0, &solana_sdk::system_program::ID); // More lamports

    let current_operator = Pubkey::new_unique();
    let current_operator_account =
        Account::new(10 * LAMPORTS_PER_SOL, 0, &solana_sdk::system_program::ID);

    let new_operator = Pubkey::new_unique();
    let new_operator_account =
        Account::new(10 * LAMPORTS_PER_SOL, 0, &solana_sdk::system_program::ID);

    let chain_name = "solana".to_owned();
    let its_hub_address = "0x123456789abcdef".to_owned();

    let (its_root_pda, its_root_account, _, _, _, _) = init_its_service(
        &mollusk,
        payer,
        &payer_account,
        upgrade_authority,
        current_operator,
        &current_operator_account,
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
    let token_id = interchain_token_id(&current_operator, &salt);
    let (token_manager_pda, _) = TokenManager::find_pda(token_id, its_root_pda);
    let (minter_roles_pda, _) = UserRoles::find_pda(&token_manager_pda, &current_operator);

    let ctx = DeployInterchainTokenContext::new(
        mollusk,
        (its_root_pda, its_root_account.clone()),
        (current_operator, current_operator_account.clone()),
        (payer, payer_account.clone()),
        Some(current_operator), // minter (will get OPERATOR role for token manager)
        Some(minter_roles_pda), // minter_roles_pda
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

    let current_operator_token_roles_account = deploy_result
        .get_account(&minter_roles_pda)
        .expect("Current operator token roles account should exist");

    let current_operator_token_roles =
        UserRoles::try_deserialize(&mut current_operator_token_roles_account.data.as_slice())
            .expect("Failed to deserialize current operator token roles");
    assert!(
        current_operator_token_roles.roles.contains(Roles::OPERATOR),
        "Current operator should have OPERATOR role for token manager"
    );

    let (new_operator_token_roles_pda, new_operator_token_roles_pda_bump) =
        UserRoles::find_pda(&token_manager_pda, &new_operator);

    let ix = Instruction {
        program_id,
        accounts: solana_axelar_its::accounts::TransferTokenManagerOperatorship {
            system_program: solana_sdk::system_program::ID,
            payer,
            origin_user_account: current_operator,
            origin_roles_account: minter_roles_pda,
            its_root_pda,
            token_manager_account: token_manager_pda,
            destination_user_account: new_operator,
            destination_roles_account: new_operator_token_roles_pda,
        }
        .to_account_metas(None),
        data: solana_axelar_its::instruction::TransferTokenManagerOperatorship {}.data(),
    };

    let accounts = vec![
        keyed_account_for_system_program(),
        (payer, payer_account.clone()),
        (current_operator, current_operator_account.clone()),
        (
            minter_roles_pda,
            current_operator_token_roles_account.clone(),
        ),
        (its_root_pda, its_root_account.clone()),
        (token_manager_pda, token_manager_account.clone()),
        (new_operator, new_operator_account.clone()),
        (
            new_operator_token_roles_pda,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
    ];

    let result = mollusk.process_instruction(&ix, &accounts);

    assert!(result.program_result.is_ok());

    let old_operator_token_roles_account = result
        .get_account(&minter_roles_pda)
        .expect("Current operator token roles account should exist");

    let old_operator_token_roles =
        UserRoles::try_deserialize(&mut old_operator_token_roles_account.data.as_slice())
            .expect("Failed to deserialize current operator token roles");
    assert!(
        !old_operator_token_roles.roles.contains(Roles::OPERATOR),
        "Old operator should not have OPERATOR role for token manager"
    );

    let updated_current_token_roles_account = result
        .get_account(&minter_roles_pda)
        .expect("Current operator token roles account should exist");

    let updated_current_token_roles =
        UserRoles::try_deserialize(&mut updated_current_token_roles_account.data.as_slice())
            .expect("Failed to deserialize updated current operator token roles");

    assert!(
        !updated_current_token_roles.roles.contains(Roles::OPERATOR),
        "Current operator should no longer have OPERATOR role for token manager"
    );

    let new_operator_token_roles_account = result
        .get_account(&new_operator_token_roles_pda)
        .expect("New operator token roles account should exist");

    let new_operator_token_roles =
        UserRoles::try_deserialize(&mut new_operator_token_roles_account.data.as_slice())
            .expect("Failed to deserialize new operator token roles");

    assert_eq!(
        new_operator_token_roles.bump,
        new_operator_token_roles_pda_bump
    );

    assert!(new_operator_token_roles.roles.contains(Roles::OPERATOR));
}

#[test]
fn test_reject_transfer_token_manager_operatorship_with_unauthorized_operator() {
    let program_id = solana_axelar_its::id();
    let mollusk = initialize_mollusk();

    let upgrade_authority = Pubkey::new_unique();
    let payer = upgrade_authority;
    let payer_account = Account::new(10 * LAMPORTS_PER_SOL, 0, &solana_sdk::system_program::ID); // More lamports

    let current_operator = Pubkey::new_unique();
    let current_operator_account =
        Account::new(10 * LAMPORTS_PER_SOL, 0, &solana_sdk::system_program::ID);

    let new_operator = Pubkey::new_unique();
    let new_operator_account =
        Account::new(10 * LAMPORTS_PER_SOL, 0, &solana_sdk::system_program::ID);

    let chain_name = "solana".to_owned();
    let its_hub_address = "0x123456789abcdef".to_owned();

    let (its_root_pda, its_root_account, _, _, _, _) = init_its_service(
        &mollusk,
        payer,
        &payer_account,
        upgrade_authority,
        current_operator,
        &current_operator_account,
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
    let token_id = interchain_token_id(&current_operator, &salt);
    let (token_manager_pda, _) = TokenManager::find_pda(token_id, its_root_pda);
    let (minter_roles_pda, _) = UserRoles::find_pda(&token_manager_pda, &current_operator);

    let ctx = DeployInterchainTokenContext::new(
        mollusk,
        (its_root_pda, its_root_account.clone()),
        (current_operator, current_operator_account.clone()),
        (payer, payer_account.clone()),
        Some(current_operator), // minter (will get OPERATOR role for token manager)
        Some(minter_roles_pda), // minter_roles_pda
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

    let current_operator_token_roles_account = deploy_result
        .get_account(&minter_roles_pda)
        .expect("Current operator token roles account should exist");

    let current_operator_token_roles =
        UserRoles::try_deserialize(&mut current_operator_token_roles_account.data.as_slice())
            .expect("Failed to deserialize current operator token roles");
    assert!(
        current_operator_token_roles.roles.contains(Roles::OPERATOR),
        "Current operator should have OPERATOR role for token manager"
    );

    let (new_operator_token_roles_pda, _) = UserRoles::find_pda(&token_manager_pda, &new_operator);

    let malicious_operator = Pubkey::new_unique();

    let ix = Instruction {
        program_id,
        accounts: solana_axelar_its::accounts::TransferTokenManagerOperatorship {
            system_program: solana_sdk::system_program::ID,
            payer,
            origin_user_account: malicious_operator,
            origin_roles_account: minter_roles_pda,
            its_root_pda,
            token_manager_account: token_manager_pda,
            destination_user_account: new_operator,
            destination_roles_account: new_operator_token_roles_pda,
        }
        .to_account_metas(None),
        data: solana_axelar_its::instruction::TransferTokenManagerOperatorship {}.data(),
    };

    let accounts = vec![
        keyed_account_for_system_program(),
        (payer, payer_account.clone()),
        (malicious_operator, current_operator_account.clone()),
        (
            minter_roles_pda,
            current_operator_token_roles_account.clone(),
        ),
        (its_root_pda, its_root_account.clone()),
        (token_manager_pda, token_manager_account.clone()),
        (new_operator, new_operator_account.clone()),
        (
            new_operator_token_roles_pda,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
    ];

    let checks = vec![Check::err(
        anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::ConstraintSeeds).into(),
    )];

    mollusk.process_and_validate_instruction(&ix, &accounts, &checks);
}

#[test]
fn test_reject_transfer_token_manager_operatorship_without_operator_role() {
    let program_id = solana_axelar_its::id();
    let mollusk = initialize_mollusk();

    let upgrade_authority = Pubkey::new_unique();
    let payer = upgrade_authority;
    let payer_account = Account::new(10 * LAMPORTS_PER_SOL, 0, &solana_sdk::system_program::ID); // More lamports

    let current_operator = Pubkey::new_unique();
    let current_operator_account =
        Account::new(10 * LAMPORTS_PER_SOL, 0, &solana_sdk::system_program::ID);

    let new_operator = Pubkey::new_unique();
    let new_operator_account =
        Account::new(10 * LAMPORTS_PER_SOL, 0, &solana_sdk::system_program::ID);

    let chain_name = "solana".to_owned();
    let its_hub_address = "0x123456789abcdef".to_owned();

    let (its_root_pda, its_root_account, _, _, _, _) = init_its_service(
        &mollusk,
        payer,
        &payer_account,
        upgrade_authority,
        current_operator,
        &current_operator_account,
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
    let token_id = interchain_token_id(&current_operator, &salt);
    let (token_manager_pda, _) = TokenManager::find_pda(token_id, its_root_pda);
    let (minter_roles_pda, _) = UserRoles::find_pda(&token_manager_pda, &current_operator);

    let ctx = DeployInterchainTokenContext::new(
        mollusk,
        (its_root_pda, its_root_account.clone()),
        (current_operator, current_operator_account.clone()),
        (payer, payer_account.clone()),
        Some(current_operator), // minter (will get OPERATOR role for token manager)
        Some(minter_roles_pda), // minter_roles_pda
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

    let current_operator_token_roles_account = deploy_result
        .get_account(&minter_roles_pda)
        .expect("Current operator token roles account should exist");

    // Remove roles from minter account
    let mut current_operator_token_roles_account_clone =
        current_operator_token_roles_account.clone();

    let mut current_operator_token_roles =
        UserRoles::try_deserialize(&mut current_operator_token_roles_account_clone.data.as_ref())
            .expect("Failed to deserialize flow limiter roles");
    current_operator_token_roles.roles = Roles::empty();

    let mut new_data = Vec::new();
    new_data.extend_from_slice(UserRoles::DISCRIMINATOR);
    current_operator_token_roles
        .serialize(&mut new_data)
        .expect("Failed to serialize");
    current_operator_token_roles_account_clone.data = new_data;

    let (new_operator_token_roles_pda, _) = UserRoles::find_pda(&token_manager_pda, &new_operator);

    let ix = Instruction {
        program_id,
        accounts: solana_axelar_its::accounts::TransferTokenManagerOperatorship {
            system_program: solana_sdk::system_program::ID,
            payer,
            origin_user_account: current_operator,
            origin_roles_account: minter_roles_pda,
            its_root_pda,
            token_manager_account: token_manager_pda,
            destination_user_account: new_operator,
            destination_roles_account: new_operator_token_roles_pda,
        }
        .to_account_metas(None),
        data: solana_axelar_its::instruction::TransferTokenManagerOperatorship {}.data(),
    };

    let accounts = vec![
        keyed_account_for_system_program(),
        (payer, payer_account.clone()),
        (current_operator, current_operator_account.clone()),
        (minter_roles_pda, current_operator_token_roles_account_clone),
        (its_root_pda, its_root_account.clone()),
        (token_manager_pda, token_manager_account.clone()),
        (new_operator, new_operator_account.clone()),
        (
            new_operator_token_roles_pda,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
    ];

    let checks = vec![Check::err(
        anchor_lang::error::Error::from(RolesError::MissingOperatorRole).into(),
    )];

    mollusk.process_and_validate_instruction(&ix, &accounts, &checks);
}
