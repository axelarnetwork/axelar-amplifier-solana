#![cfg(test)]
#![allow(clippy::too_many_lines)]

use anchor_lang::{
    solana_program::instruction::Instruction, AccountDeserialize, AnchorSerialize, Discriminator,
    InstructionData, ToAccountMetas,
};
use mollusk_svm::{program::keyed_account_for_system_program, result::Check};
use solana_axelar_its::state::{Roles, RolesError, TokenManager, UserRoles};
use solana_axelar_its_test_fixtures::{
    deploy_interchain_token_helper, init_its_service, initialize_mollusk_with_programs,
    new_default_account, new_empty_account, new_test_account, DeployInterchainTokenContext,
};
use solana_sdk::pubkey::Pubkey;

#[test]
fn remove_token_manager_flow_limiter() {
    let program_id = solana_axelar_its::id();
    let mollusk = initialize_mollusk_with_programs();

    let (payer, payer_account) = new_test_account();
    let (deployer, deployer_account) = new_test_account();
    let (operator, operator_account) = new_test_account();

    let chain_name = "solana".to_owned();
    let its_hub_address = "0x123456789abcdef".to_owned();

    // Initialize ITS service first
    let (its_root_pda, its_root_account, _, _, _, _, _, _) = init_its_service(
        &mollusk,
        payer,
        &payer_account,
        payer,
        operator,
        &operator_account,
        chain_name.clone(),
        its_hub_address.clone(),
    );

    // Deploy an interchain token
    let salt = [1u8; 32];
    let name = "Test Token".to_owned();
    let symbol = "TEST".to_owned();
    let decimals = 9u8;
    let initial_supply = 1_000_000_000u64;

    let minter = Pubkey::new_unique();

    let token_id = solana_axelar_its::utils::interchain_token_id(&deployer, &salt);
    let (token_manager_pda, _token_manager_bump) = TokenManager::find_pda(token_id, its_root_pda);

    // The minter in interchain tokens gets all 3 roles (Operator, Minter, FlowLimiter)
    let (minter_roles_pda, _minter_roles_pda_bump) =
        UserRoles::find_pda(&token_manager_pda, &minter);

    let ctx = DeployInterchainTokenContext::new(
        mollusk,
        (its_root_pda, its_root_account.clone()),
        (deployer, deployer_account),
        (payer, payer_account.clone()),
        Some(minter),
        Some(minter_roles_pda),
    );

    let (deploy_result, token_manager_pda, _, _, _, _, mollusk) = deploy_interchain_token_helper(
        ctx,
        salt,
        name,
        symbol,
        decimals,
        initial_supply,
        vec![Check::success()],
    );

    assert!(
        deploy_result.program_result.is_ok(),
        "Deploy interchain token instruction should succeed: {:?}",
        deploy_result.program_result
    );

    let (flow_limiter_user, flow_limiter_user_account) = new_test_account();
    let (flow_limiter_roles_pda, _) = UserRoles::find_pda(&token_manager_pda, &flow_limiter_user);

    let add_flow_limiter_ix = Instruction {
        program_id,
        accounts: solana_axelar_its::accounts::AddTokenManagerFlowLimiter {
            system_program: solana_sdk::system_program::ID,
            payer,
            authority_user_account: minter, // use interchain token minter which is also the operator
            authority_roles_account: minter_roles_pda,
            its_root_pda,
            token_manager_pda,
            target_user_account: flow_limiter_user,
            target_roles_account: flow_limiter_roles_pda,
        }
        .to_account_metas(None),
        data: solana_axelar_its::instruction::AddTokenManagerFlowLimiter {}.data(),
    };

    let minter_roles_account = deploy_result
        .get_account(&minter_roles_pda)
        .expect("Minter roles account should exist after token deployment");

    let minter_account = new_default_account();

    let add_flow_limiter_accounts = vec![
        keyed_account_for_system_program(),
        (payer, payer_account.clone()),
        (minter, minter_account.clone()),
        (minter_roles_pda, minter_roles_account.clone()),
        (its_root_pda, its_root_account.clone()),
        (
            token_manager_pda,
            deploy_result
                .get_account(&token_manager_pda)
                .unwrap()
                .clone(),
        ),
        (flow_limiter_user, flow_limiter_user_account.clone()),
        (flow_limiter_roles_pda, new_empty_account()),
    ];

    let add_flow_limiter_result =
        mollusk.process_instruction(&add_flow_limiter_ix, &add_flow_limiter_accounts);

    assert!(add_flow_limiter_result.program_result.is_ok());

    // Verify the flow limiter role was added
    let flow_limiter_roles_account = add_flow_limiter_result
        .get_account(&flow_limiter_roles_pda)
        .expect("Flow limiter roles account should exist after adding flow limiter");

    let flow_limiter_roles =
        UserRoles::try_deserialize(&mut flow_limiter_roles_account.data.as_ref())
            .expect("Failed to deserialize flow limiter roles");

    assert!(
        flow_limiter_roles.has_flow_limiter_role(),
        "User should have FLOW_LIMITER role after adding"
    );

    // Now test removing the flow limiter role
    let remove_flow_limiter_ix = Instruction {
        program_id,
        accounts: solana_axelar_its::accounts::RemoveTokenManagerFlowLimiter {
            system_program: solana_sdk::system_program::ID,
            payer,
            authority_user_account: minter, // use interchain token minter which is also the operator
            authority_roles_account: minter_roles_pda,
            its_root_pda,
            token_manager_pda,
            target_user_account: flow_limiter_user,
            target_roles_account: flow_limiter_roles_pda,
        }
        .to_account_metas(None),
        data: solana_axelar_its::instruction::RemoveTokenManagerFlowLimiter {}.data(),
    };

    let remove_flow_limiter_accounts = vec![
        keyed_account_for_system_program(),
        (payer, payer_account.clone()),
        (minter, minter_account.clone()),
        (minter_roles_pda, minter_roles_account.clone()),
        (its_root_pda, its_root_account.clone()),
        (
            token_manager_pda,
            add_flow_limiter_result
                .get_account(&token_manager_pda)
                .unwrap()
                .clone(),
        ),
        (flow_limiter_user, flow_limiter_user_account.clone()),
        (flow_limiter_roles_pda, flow_limiter_roles_account.clone()),
    ];

    let remove_flow_limiter_result =
        mollusk.process_instruction(&remove_flow_limiter_ix, &remove_flow_limiter_accounts);

    assert!(remove_flow_limiter_result.program_result.is_ok());

    // Verify that the flow limiter role was removed correctly
    let updated_flow_limiter_roles_account = remove_flow_limiter_result
        .get_account(&flow_limiter_roles_pda)
        .expect("Flow limiter roles account should exist after removing flow limiter");

    // Check that account was closed
    assert!(updated_flow_limiter_roles_account.data.is_empty());
}

#[test]
fn reject_remove_token_manager_flow_limiter_with_unauthorized_authority() {
    let program_id = solana_axelar_its::id();
    let mollusk = initialize_mollusk_with_programs();

    let (payer, payer_account) = new_test_account();
    let (deployer, deployer_account) = new_test_account();
    let (operator, operator_account) = new_test_account();

    let chain_name = "solana".to_owned();
    let its_hub_address = "0x123456789abcdef".to_owned();

    // Initialize ITS service first
    let (its_root_pda, its_root_account, _, _, _, _, _, _) = init_its_service(
        &mollusk,
        payer,
        &payer_account,
        payer,
        operator,
        &operator_account,
        chain_name.clone(),
        its_hub_address.clone(),
    );

    // Deploy an interchain token
    let salt = [1u8; 32];
    let name = "Test Token".to_owned();
    let symbol = "TEST".to_owned();
    let decimals = 9u8;
    let initial_supply = 1_000_000_000u64;

    let minter = Pubkey::new_unique();

    let token_id = solana_axelar_its::utils::interchain_token_id(&deployer, &salt);
    let (token_manager_pda, _token_manager_bump) = TokenManager::find_pda(token_id, its_root_pda);

    // The minter in interchain tokens gets all 3 roles (Operator, Minter, FlowLimiter)
    let (minter_roles_pda, _minter_roles_pda_bump) =
        UserRoles::find_pda(&token_manager_pda, &minter);

    let ctx = DeployInterchainTokenContext::new(
        mollusk,
        (its_root_pda, its_root_account.clone()),
        (deployer, deployer_account),
        (payer, payer_account.clone()),
        Some(minter),
        Some(minter_roles_pda),
    );

    let (deploy_result, token_manager_pda, _, _, _, _, mollusk) = deploy_interchain_token_helper(
        ctx,
        salt,
        name,
        symbol,
        decimals,
        initial_supply,
        vec![Check::success()],
    );

    assert!(
        deploy_result.program_result.is_ok(),
        "Deploy interchain token instruction should succeed: {:?}",
        deploy_result.program_result
    );

    let (flow_limiter_user, flow_limiter_user_account) = new_test_account();
    let (flow_limiter_roles_pda, _) = UserRoles::find_pda(&token_manager_pda, &flow_limiter_user);

    let add_flow_limiter_ix = Instruction {
        program_id,
        accounts: solana_axelar_its::accounts::AddTokenManagerFlowLimiter {
            system_program: solana_sdk::system_program::ID,
            payer,
            authority_user_account: minter, // use interchain token minter which is also the operator
            authority_roles_account: minter_roles_pda,
            its_root_pda,
            token_manager_pda,
            target_user_account: flow_limiter_user,
            target_roles_account: flow_limiter_roles_pda,
        }
        .to_account_metas(None),
        data: solana_axelar_its::instruction::AddTokenManagerFlowLimiter {}.data(),
    };

    let minter_roles_account = deploy_result
        .get_account(&minter_roles_pda)
        .expect("Minter roles account should exist after token deployment");

    let minter_account = new_default_account();

    let add_flow_limiter_accounts = vec![
        keyed_account_for_system_program(),
        (payer, payer_account.clone()),
        (minter, minter_account.clone()),
        (minter_roles_pda, minter_roles_account.clone()),
        (its_root_pda, its_root_account.clone()),
        (
            token_manager_pda,
            deploy_result
                .get_account(&token_manager_pda)
                .unwrap()
                .clone(),
        ),
        (flow_limiter_user, flow_limiter_user_account.clone()),
        (flow_limiter_roles_pda, new_empty_account()),
    ];

    let add_flow_limiter_result =
        mollusk.process_instruction(&add_flow_limiter_ix, &add_flow_limiter_accounts);

    assert!(add_flow_limiter_result.program_result.is_ok());

    // Verify the flow limiter role was added
    let flow_limiter_roles_account = add_flow_limiter_result
        .get_account(&flow_limiter_roles_pda)
        .expect("Flow limiter roles account should exist after adding flow limiter");

    let flow_limiter_roles =
        UserRoles::try_deserialize(&mut flow_limiter_roles_account.data.as_ref())
            .expect("Failed to deserialize flow limiter roles");

    assert!(
        flow_limiter_roles.has_flow_limiter_role(),
        "User should have FLOW_LIMITER role after adding"
    );

    let malicious_minter = Pubkey::new_unique();
    let malicious_minter_account = new_default_account();

    // Now test removing the flow limiter role
    let remove_flow_limiter_ix = Instruction {
        program_id,
        accounts: solana_axelar_its::accounts::RemoveTokenManagerFlowLimiter {
            system_program: solana_sdk::system_program::ID,
            payer,
            authority_user_account: malicious_minter, // use interchain token minter which is also the operator
            authority_roles_account: minter_roles_pda,
            its_root_pda,
            token_manager_pda,
            target_user_account: flow_limiter_user,
            target_roles_account: flow_limiter_roles_pda,
        }
        .to_account_metas(None),
        data: solana_axelar_its::instruction::RemoveTokenManagerFlowLimiter {}.data(),
    };

    let remove_flow_limiter_accounts = vec![
        keyed_account_for_system_program(),
        (payer, payer_account.clone()),
        (malicious_minter, malicious_minter_account),
        (minter_roles_pda, minter_roles_account.clone()),
        (its_root_pda, its_root_account.clone()),
        (
            token_manager_pda,
            add_flow_limiter_result
                .get_account(&token_manager_pda)
                .unwrap()
                .clone(),
        ),
        (flow_limiter_user, flow_limiter_user_account.clone()),
        (flow_limiter_roles_pda, flow_limiter_roles_account.clone()),
    ];

    let checks = vec![Check::err(
        anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::ConstraintSeeds).into(),
    )];

    mollusk.process_and_validate_instruction(
        &remove_flow_limiter_ix,
        &remove_flow_limiter_accounts,
        &checks,
    );
}

#[test]
fn reject_remove_token_manager_flow_limiter_without_operator_role() {
    let program_id = solana_axelar_its::id();
    let mollusk = initialize_mollusk_with_programs();

    let (payer, payer_account) = new_test_account();
    let (deployer, deployer_account) = new_test_account();
    let (operator, operator_account) = new_test_account();

    let chain_name = "solana".to_owned();
    let its_hub_address = "0x123456789abcdef".to_owned();

    // Initialize ITS service first
    let (its_root_pda, its_root_account, _, _, _, _, _, _) = init_its_service(
        &mollusk,
        payer,
        &payer_account,
        payer,
        operator,
        &operator_account,
        chain_name.clone(),
        its_hub_address.clone(),
    );

    // Deploy an interchain token
    let salt = [1u8; 32];
    let name = "Test Token".to_owned();
    let symbol = "TEST".to_owned();
    let decimals = 9u8;
    let initial_supply = 1_000_000_000u64;

    let minter = Pubkey::new_unique();

    let token_id = solana_axelar_its::utils::interchain_token_id(&deployer, &salt);
    let (token_manager_pda, _token_manager_bump) = TokenManager::find_pda(token_id, its_root_pda);

    // The minter in interchain tokens gets all 3 roles (Operator, Minter, FlowLimiter)
    let (minter_roles_pda, _minter_roles_pda_bump) =
        UserRoles::find_pda(&token_manager_pda, &minter);

    let ctx = DeployInterchainTokenContext::new(
        mollusk,
        (its_root_pda, its_root_account.clone()),
        (deployer, deployer_account),
        (payer, payer_account.clone()),
        Some(minter),
        Some(minter_roles_pda),
    );

    let (deploy_result, token_manager_pda, _, _, _, _, mollusk) = deploy_interchain_token_helper(
        ctx,
        salt,
        name,
        symbol,
        decimals,
        initial_supply,
        vec![Check::success()],
    );

    assert!(
        deploy_result.program_result.is_ok(),
        "Deploy interchain token instruction should succeed: {:?}",
        deploy_result.program_result
    );

    let (flow_limiter_user, flow_limiter_user_account) = new_test_account();
    let (flow_limiter_roles_pda, _) = UserRoles::find_pda(&token_manager_pda, &flow_limiter_user);

    let add_flow_limiter_ix = Instruction {
        program_id,
        accounts: solana_axelar_its::accounts::AddTokenManagerFlowLimiter {
            system_program: solana_sdk::system_program::ID,
            payer,
            authority_user_account: minter, // use interchain token minter which is also the operator
            authority_roles_account: minter_roles_pda,
            its_root_pda,
            token_manager_pda,
            target_user_account: flow_limiter_user,
            target_roles_account: flow_limiter_roles_pda,
        }
        .to_account_metas(None),
        data: solana_axelar_its::instruction::AddTokenManagerFlowLimiter {}.data(),
    };

    let minter_roles_account = deploy_result
        .get_account(&minter_roles_pda)
        .expect("Minter roles account should exist after token deployment");

    let minter_account = new_default_account();

    let add_flow_limiter_accounts = vec![
        keyed_account_for_system_program(),
        (payer, payer_account.clone()),
        (minter, minter_account.clone()),
        (minter_roles_pda, minter_roles_account.clone()),
        (its_root_pda, its_root_account.clone()),
        (
            token_manager_pda,
            deploy_result
                .get_account(&token_manager_pda)
                .unwrap()
                .clone(),
        ),
        (flow_limiter_user, flow_limiter_user_account.clone()),
        (flow_limiter_roles_pda, new_empty_account()),
    ];

    let add_flow_limiter_result =
        mollusk.process_instruction(&add_flow_limiter_ix, &add_flow_limiter_accounts);

    assert!(add_flow_limiter_result.program_result.is_ok());

    // Verify the flow limiter role was added
    let flow_limiter_roles_account = add_flow_limiter_result
        .get_account(&flow_limiter_roles_pda)
        .expect("Flow limiter roles account should exist after adding flow limiter");

    let flow_limiter_roles =
        UserRoles::try_deserialize(&mut flow_limiter_roles_account.data.as_ref())
            .expect("Failed to deserialize flow limiter roles");

    assert!(
        flow_limiter_roles.has_flow_limiter_role(),
        "User should have FLOW_LIMITER role after adding"
    );

    // Remove roles from minter account
    let mut minter_roles_account_clone = minter_roles_account.clone();

    let mut minter_roles =
        UserRoles::try_deserialize(&mut minter_roles_account_clone.data.as_ref())
            .expect("Failed to deserialize flow limiter roles");
    minter_roles.roles = Roles::empty();

    let mut new_data = Vec::new();
    new_data.extend_from_slice(UserRoles::DISCRIMINATOR);
    minter_roles
        .serialize(&mut new_data)
        .expect("Failed to serialize");
    minter_roles_account_clone.data = new_data;

    // Now test removing the flow limiter role
    let remove_flow_limiter_ix = Instruction {
        program_id,
        accounts: solana_axelar_its::accounts::RemoveTokenManagerFlowLimiter {
            system_program: solana_sdk::system_program::ID,
            payer,
            authority_user_account: minter, // use interchain token minter which is also the operator
            authority_roles_account: minter_roles_pda,
            its_root_pda,
            token_manager_pda,
            target_user_account: flow_limiter_user,
            target_roles_account: flow_limiter_roles_pda,
        }
        .to_account_metas(None),
        data: solana_axelar_its::instruction::RemoveTokenManagerFlowLimiter {}.data(),
    };

    let remove_flow_limiter_accounts = vec![
        keyed_account_for_system_program(),
        (payer, payer_account.clone()),
        (minter, minter_account.clone()),
        (minter_roles_pda, minter_roles_account_clone.clone()),
        (its_root_pda, its_root_account.clone()),
        (
            token_manager_pda,
            add_flow_limiter_result
                .get_account(&token_manager_pda)
                .unwrap()
                .clone(),
        ),
        (flow_limiter_user, flow_limiter_user_account.clone()),
        (flow_limiter_roles_pda, flow_limiter_roles_account.clone()),
    ];

    let checks = vec![Check::err(
        anchor_lang::error::Error::from(RolesError::MissingOperatorRole).into(),
    )];

    mollusk.process_and_validate_instruction(
        &remove_flow_limiter_ix,
        &remove_flow_limiter_accounts,
        &checks,
    );
}
