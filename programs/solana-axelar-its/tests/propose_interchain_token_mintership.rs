#![cfg(test)]
#![allow(clippy::too_many_lines)]

use anchor_lang::{AccountDeserialize, AnchorSerialize, Discriminator};
use mollusk_svm::result::Check;
use solana_axelar_its::state::{RoleProposal, roles, RolesError, TokenManager, Type, UserRoles};
use solana_axelar_its::utils::interchain_token_id;
use solana_axelar_its_test_fixtures::{
    deploy_interchain_token_helper, init_its_service, initialize_mollusk_with_programs,
    new_test_account, propose_interchain_token_mintership_helper, DeployInterchainTokenContext,
    ProposeInterchainTokenMintershipContext,
};
use solana_sdk::pubkey::Pubkey;

#[test]
fn test_propose_interchain_token_mintership() {
    let program_id = solana_axelar_its::id();
    let mollusk = initialize_mollusk_with_programs();

    let (upgrade_authority, _) = new_test_account();
    let payer = upgrade_authority;
    let (_, payer_account) = new_test_account(); // Get a fresh account for payer

    let (current_minter, current_minter_account) = new_test_account();
    let (proposed_minter, proposed_minter_account) = new_test_account();
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

    let token_id = interchain_token_id(&current_minter, &salt);
    let (token_manager_pda, _) = TokenManager::find_pda(token_id, its_root_pda);
    let (minter_roles_pda, _) = UserRoles::find_pda(&token_manager_pda, &current_minter);

    let ctx = DeployInterchainTokenContext::new(
        mollusk,
        (its_root_pda, its_root_account.clone()),
        (current_minter, current_minter_account.clone()),
        (payer, payer_account.clone()),
        Some(current_minter),
        Some(minter_roles_pda),
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

    // Verify the token manager is of type NativeInterchainToken
    let token_manager_data =
        TokenManager::try_deserialize(&mut token_manager_account.data.as_slice())
            .expect("Failed to deserialize TokenManager");
    assert_eq!(token_manager_data.ty, Type::NativeInterchainToken);

    let current_minter_token_roles_account = deploy_result
        .get_account(&minter_roles_pda)
        .expect("Current minter token roles account should exist");

    let current_minter_token_roles =
        UserRoles::try_deserialize(&mut current_minter_token_roles_account.data.as_slice())
            .expect("Failed to deserialize current minter token roles");

    assert!(current_minter_token_roles.contains(roles::MINTER));

    // Nonexistent account, will be deployed by ProposeInterchainTokenMintership
    let (proposal_pda, proposal_pda_bump) = RoleProposal::find_pda(
        &token_manager_pda,
        &current_minter,
        &proposed_minter,
        &program_id,
    );

    let ctx = ProposeInterchainTokenMintershipContext::new(
        mollusk,
        (payer, payer_account.clone()),
        (current_minter, current_minter_account.clone()),
        (minter_roles_pda, current_minter_token_roles_account.clone()),
        (its_root_pda, its_root_account.clone()),
        (token_manager_pda, token_manager_account.clone()),
        (proposed_minter, proposed_minter_account.clone()),
    );

    let checks = vec![Check::success()];
    let (result, _) = propose_interchain_token_mintership_helper(ctx, checks);

    assert!(result.program_result.is_ok());

    // Verify the proposal account was created with correct data
    let proposal_account = result
        .get_account(&proposal_pda)
        .expect("Proposal account should exist");

    let proposal_data = RoleProposal::try_deserialize(&mut proposal_account.data.as_slice())
        .expect("Failed to deserialize RoleProposal");

    assert_eq!(proposal_data.roles, roles::MINTER);
    assert_eq!(proposal_data.bump, proposal_pda_bump);

    // Verify the current minter still has their role (proposal doesn't transfer immediately)
    let current_minter_token_roles_account_after = result
        .get_account(&minter_roles_pda)
        .expect("Current minter token roles account should exist");

    let current_minter_token_roles_after =
        UserRoles::try_deserialize(&mut current_minter_token_roles_account_after.data.as_slice())
            .expect("Failed to deserialize current minter token roles after proposal");

    assert!(current_minter_token_roles_after.contains(roles::MINTER));
}

#[test]
fn test_reject_propose_interchain_token_mintership_with_invalid_authority() {
    let mollusk = initialize_mollusk_with_programs();

    let (upgrade_authority, _) = new_test_account();
    let payer = upgrade_authority;
    let (_, payer_account) = new_test_account(); // Get a fresh account for payer

    let (current_minter, current_minter_account) = new_test_account();
    let (proposed_minter, proposed_minter_account) = new_test_account();
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

    let token_id = interchain_token_id(&current_minter, &salt);
    let (token_manager_pda, _) = TokenManager::find_pda(token_id, its_root_pda);
    let (minter_roles_pda, _) = UserRoles::find_pda(&token_manager_pda, &current_minter);

    let ctx = DeployInterchainTokenContext::new(
        mollusk,
        (its_root_pda, its_root_account.clone()),
        (current_minter, current_minter_account.clone()),
        (payer, payer_account.clone()),
        Some(current_minter),
        Some(minter_roles_pda),
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
        .get_account(&minter_roles_pda)
        .expect("Current minter token roles account should exist");

    let current_minter_token_roles =
        UserRoles::try_deserialize(&mut current_minter_token_roles_account.data.as_slice())
            .expect("Failed to deserialize current minter token roles");

    assert!(current_minter_token_roles.contains(roles::MINTER));

    let invalid_current_minter = Pubkey::new_unique();

    let ctx = ProposeInterchainTokenMintershipContext::new(
        mollusk,
        (payer, payer_account.clone()),
        (invalid_current_minter, current_minter_account.clone()),
        (minter_roles_pda, current_minter_token_roles_account.clone()),
        (its_root_pda, its_root_account.clone()),
        (token_manager_pda, token_manager_account.clone()),
        (proposed_minter, proposed_minter_account.clone()),
    );

    let checks = vec![Check::err(
        anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::ConstraintSeeds).into(),
    )];

    let (result, _) = propose_interchain_token_mintership_helper(ctx, checks);
    assert!(result.program_result.is_err());
}

#[test]
fn test_reject_propose_interchain_token_mintership_without_minter_role() {
    let mollusk = initialize_mollusk_with_programs();

    let (upgrade_authority, _) = new_test_account();
    let payer = upgrade_authority;
    let (_, payer_account) = new_test_account(); // Get a fresh account for payer

    let (current_minter, current_minter_account) = new_test_account();
    let (proposed_minter, proposed_minter_account) = new_test_account();
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

    let token_id = interchain_token_id(&current_minter, &salt);
    let (token_manager_pda, _) = TokenManager::find_pda(token_id, its_root_pda);
    let (minter_roles_pda, _) = UserRoles::find_pda(&token_manager_pda, &current_minter);

    let ctx = DeployInterchainTokenContext::new(
        mollusk,
        (its_root_pda, its_root_account.clone()),
        (current_minter, current_minter_account.clone()),
        (payer, payer_account.clone()),
        Some(current_minter),
        Some(minter_roles_pda),
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
        .get_account(&minter_roles_pda)
        .expect("Current minter token roles account should exist");

    let mut current_minter_token_roles_account_clone = current_minter_token_roles_account.clone();

    let mut current_minter_token_roles =
        UserRoles::try_deserialize(&mut current_minter_token_roles_account_clone.data.as_slice())
            .expect("Failed to deserialize current minter token roles");

    current_minter_token_roles.roles = roles::EMPTY;

    // Remove roles from current minter
    let mut new_data = Vec::new();
    new_data.extend_from_slice(UserRoles::DISCRIMINATOR);
    current_minter_token_roles
        .serialize(&mut new_data)
        .expect("Failed to serialize");
    current_minter_token_roles_account_clone.data = new_data;

    let ctx = ProposeInterchainTokenMintershipContext::new(
        mollusk,
        (payer, payer_account.clone()),
        (current_minter, current_minter_account.clone()),
        (
            minter_roles_pda,
            current_minter_token_roles_account_clone.clone(),
        ),
        (its_root_pda, its_root_account.clone()),
        (token_manager_pda, token_manager_account.clone()),
        (proposed_minter, proposed_minter_account.clone()),
    );

    let checks = vec![Check::err(
        anchor_lang::error::Error::from(RolesError::MissingMinterRole).into(),
    )];

    let (result, _) = propose_interchain_token_mintership_helper(ctx, checks);
    assert!(result.program_result.is_err());
}

#[test]
fn test_reject_propose_interchain_token_mintership_same_origin_destination() {
    let mollusk = initialize_mollusk_with_programs();

    let (upgrade_authority, _) = new_test_account();
    let payer = upgrade_authority;
    let (_, payer_account) = new_test_account(); // Get a fresh account for payer

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

    let token_id = interchain_token_id(&current_minter, &salt);
    let (token_manager_pda, _) = TokenManager::find_pda(token_id, its_root_pda);
    let (minter_roles_pda, _) = UserRoles::find_pda(&token_manager_pda, &current_minter);

    let ctx = DeployInterchainTokenContext::new(
        mollusk,
        (its_root_pda, its_root_account.clone()),
        (current_minter, current_minter_account.clone()),
        (payer, payer_account.clone()),
        Some(current_minter),
        Some(minter_roles_pda),
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
        .get_account(&minter_roles_pda)
        .expect("Current minter token roles account should exist");

    let ctx = ProposeInterchainTokenMintershipContext::new(
        mollusk,
        (payer, payer_account.clone()),
        (current_minter, current_minter_account.clone()),
        (minter_roles_pda, current_minter_token_roles_account.clone()),
        (its_root_pda, its_root_account.clone()),
        (token_manager_pda, token_manager_account.clone()),
        (current_minter, current_minter_account.clone()), // Same as origin
    );

    let checks = vec![Check::err(
        anchor_lang::error::Error::from(solana_axelar_its::ItsError::InvalidArgument).into(),
    )];

    let (result, _) = propose_interchain_token_mintership_helper(ctx, checks);
    assert!(result.program_result.is_err());
}
