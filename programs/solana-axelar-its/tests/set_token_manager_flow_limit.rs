#![cfg(test)]
#![allow(clippy::too_many_lines)]

use anchor_lang::{solana_program, AnchorSerialize, Discriminator};
use anchor_lang::{AccountDeserialize, InstructionData, ToAccountMetas};
use mollusk_svm::program::keyed_account_for_system_program;
use mollusk_svm::result::Check;
use mollusk_test_utils::get_event_authority_and_program_accounts;
use solana_axelar_its::state::{roles, RolesError};
use solana_axelar_its::{
    state::{TokenManager, UserRoles},
    utils::interchain_token_id,
};
use solana_axelar_its_test_fixtures::{
    deploy_interchain_token_helper, init_its_service, initialize_mollusk_with_programs,
    new_default_account, new_empty_account, new_test_account, DeployInterchainTokenContext,
};
use solana_program::instruction::Instruction;
use solana_sdk::pubkey::Pubkey;

#[test]
fn set_token_manager_flow_limit_success() {
    let program_id = solana_axelar_its::id();
    let mollusk = initialize_mollusk_with_programs();

    let (payer, payer_account) = new_test_account();
    let (deployer, deployer_account) = new_test_account();
    let (operator, operator_account) = new_test_account();

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

    // Deploy an interchain token first
    let salt = [1u8; 32];
    let name = "Test Token".to_owned();
    let symbol = "TEST".to_owned();
    let decimals = 9u8;
    let initial_supply = 1_000_000_000u64; // 1 billion tokens with 9 decimals

    let minter = Pubkey::new_unique();

    let token_id = interchain_token_id(&deployer, &salt);
    let (token_manager_pda, _) = TokenManager::find_pda(token_id, its_root_pda);
    let (minter_roles_pda, _) = UserRoles::find_pda(&token_manager_pda, &minter);

    let ctx = DeployInterchainTokenContext::new(
        mollusk,
        (its_root_pda, its_root_account.clone()),
        (deployer, deployer_account),
        (payer, payer_account.clone()),
        Some(minter),
        Some(minter_roles_pda),
    );

    let (result, token_manager_pda, _, _, _, _, mollusk) = deploy_interchain_token_helper(
        ctx,
        salt,
        name.clone(),
        symbol.clone(),
        decimals,
        initial_supply,
        vec![Check::success()],
    );

    assert!(result.program_result.is_ok());

    // Verify the token manager was created with no flow limit initially
    let token_manager_account = result.get_account(&token_manager_pda).unwrap();
    let token_manager =
        TokenManager::try_deserialize(&mut token_manager_account.data.as_ref()).unwrap();

    assert_eq!(token_manager.flow_slot.flow_limit, None);

    // Add a dedicated flow limiter user (different from minter/operator)
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

    let minter_roles_account = result
        .get_account(&minter_roles_pda)
        .expect("Minter roles account should exist after token deployment");

    let minter_account = new_default_account();

    let add_flow_limiter_accounts = vec![
        keyed_account_for_system_program(),
        (payer, payer_account.clone()),
        (minter, minter_account),
        (minter_roles_pda, minter_roles_account.clone()),
        (its_root_pda, its_root_account.clone()),
        (
            token_manager_pda,
            result.get_account(&token_manager_pda).unwrap().clone(),
        ),
        (flow_limiter_user, flow_limiter_user_account.clone()),
        (flow_limiter_roles_pda, new_empty_account()),
    ];

    let add_flow_limiter_result =
        mollusk.process_instruction(&add_flow_limiter_ix, &add_flow_limiter_accounts);

    assert!(add_flow_limiter_result.program_result.is_ok());

    // Verify that the flow limiter role was added correctly
    let flow_limiter_roles_account = add_flow_limiter_result
        .get_account(&flow_limiter_roles_pda)
        .expect("Flow limiter roles account should exist after adding flow limiter");

    let flow_limiter_roles =
        UserRoles::try_deserialize(&mut flow_limiter_roles_account.data.as_ref())
            .expect("Failed to deserialize flow limiter roles");

    assert!(flow_limiter_roles.has_flow_limiter_role());

    // Now set the token manager flow limit using the dedicated flow limiter
    let flow_limit: Option<u64> = Some(1_000_000_000); // 1 billion tokens

    let (event_authority, event_authority_account, program_account) =
        get_event_authority_and_program_accounts(&program_id);

    let set_token_manager_flow_limit_ix = Instruction {
        program_id,
        accounts: solana_axelar_its::accounts::SetTokenManagerFlowLimit {
            payer,
            flow_limiter: flow_limiter_user,
            its_root_pda,
            token_manager_pda,
            flow_limiter_roles_pda,
            system_program: solana_sdk::system_program::ID,
            // for emit cpi
            event_authority,
            program: program_id,
        }
        .to_account_metas(None),
        data: solana_axelar_its::instruction::SetTokenManagerFlowLimit { flow_limit }.data(),
    };

    let updated_token_manager_account = add_flow_limiter_result
        .get_account(&token_manager_pda)
        .unwrap();

    let accounts = vec![
        (payer, payer_account),
        (flow_limiter_user, flow_limiter_user_account),
        (its_root_pda, its_root_account),
        (token_manager_pda, updated_token_manager_account.clone()),
        (flow_limiter_roles_pda, flow_limiter_roles_account.clone()),
        keyed_account_for_system_program(),
        // for event cpi
        (event_authority, event_authority_account),
        (program_id, program_account),
    ];

    let result = mollusk.process_instruction(&set_token_manager_flow_limit_ix, &accounts);

    assert!(result.program_result.is_ok());

    // Verify the flow limit was set correctly
    let final_token_manager_account = result.get_account(&token_manager_pda).unwrap();
    let final_token_manager =
        TokenManager::try_deserialize(&mut final_token_manager_account.data.as_ref()).unwrap();

    assert_eq!(final_token_manager.flow_slot.flow_limit, flow_limit);

    // Verify other token manager fields remained unchanged
    assert_eq!(final_token_manager.token_id, token_manager.token_id);
    assert_eq!(
        final_token_manager.token_address,
        token_manager.token_address
    );
    assert_eq!(final_token_manager.flow_slot.flow_in, 0);
    assert_eq!(final_token_manager.flow_slot.flow_out, 0);
    assert_eq!(final_token_manager.flow_slot.epoch, 0);
}

#[test]
fn reject_set_token_manager_flow_limit_with_unauthorized_operator() {
    let program_id = solana_axelar_its::id();
    let mollusk = initialize_mollusk_with_programs();

    let (payer, payer_account) = new_test_account();
    let (deployer, deployer_account) = new_test_account();
    let (operator, operator_account) = new_test_account();

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

    // Deploy an interchain token first
    let salt = [1u8; 32];
    let name = "Test Token".to_owned();
    let symbol = "TEST".to_owned();
    let decimals = 9u8;
    let initial_supply = 1_000_000_000u64; // 1 billion tokens with 9 decimals

    let minter = Pubkey::new_unique();

    let token_id = interchain_token_id(&deployer, &salt);
    let (token_manager_pda, _) = TokenManager::find_pda(token_id, its_root_pda);
    let (minter_roles_pda, _) = UserRoles::find_pda(&token_manager_pda, &minter);

    let ctx = DeployInterchainTokenContext::new(
        mollusk,
        (its_root_pda, its_root_account.clone()),
        (deployer, deployer_account),
        (payer, payer_account.clone()),
        Some(minter),
        Some(minter_roles_pda),
    );

    let (result, token_manager_pda, _, _, _, _, mollusk) = deploy_interchain_token_helper(
        ctx,
        salt,
        name.clone(),
        symbol.clone(),
        decimals,
        initial_supply,
        vec![Check::success()],
    );

    assert!(result.program_result.is_ok());

    // Verify the token manager was created with no flow limit initially
    let token_manager_account = result.get_account(&token_manager_pda).unwrap();
    let token_manager =
        TokenManager::try_deserialize(&mut token_manager_account.data.as_ref()).unwrap();

    assert_eq!(token_manager.flow_slot.flow_limit, None);

    // Add a dedicated flow limiter user (different from minter/operator)
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

    let minter_roles_account = result
        .get_account(&minter_roles_pda)
        .expect("Minter roles account should exist after token deployment");

    let minter_account = new_default_account();

    let add_flow_limiter_accounts = vec![
        keyed_account_for_system_program(),
        (payer, payer_account.clone()),
        (minter, minter_account),
        (minter_roles_pda, minter_roles_account.clone()),
        (its_root_pda, its_root_account.clone()),
        (
            token_manager_pda,
            result.get_account(&token_manager_pda).unwrap().clone(),
        ),
        (flow_limiter_user, flow_limiter_user_account.clone()),
        (flow_limiter_roles_pda, new_empty_account()),
    ];

    let add_flow_limiter_result =
        mollusk.process_instruction(&add_flow_limiter_ix, &add_flow_limiter_accounts);

    assert!(add_flow_limiter_result.program_result.is_ok());

    // Verify that the flow limiter role was added correctly
    let flow_limiter_roles_account = add_flow_limiter_result
        .get_account(&flow_limiter_roles_pda)
        .expect("Flow limiter roles account should exist after adding flow limiter");

    let flow_limiter_roles =
        UserRoles::try_deserialize(&mut flow_limiter_roles_account.data.as_ref())
            .expect("Failed to deserialize flow limiter roles");

    assert!(flow_limiter_roles.has_flow_limiter_role());

    // Now set the token manager flow limit using the dedicated flow limiter
    let flow_limit: Option<u64> = Some(1_000_000_000); // 1 billion tokens

    let (event_authority, event_authority_account, program_account) =
        get_event_authority_and_program_accounts(&program_id);

    let malicious_flow_limiter = Pubkey::new_unique();

    let set_token_manager_flow_limit_ix = Instruction {
        program_id,
        accounts: solana_axelar_its::accounts::SetTokenManagerFlowLimit {
            payer,
            flow_limiter: malicious_flow_limiter,
            its_root_pda,
            token_manager_pda,
            flow_limiter_roles_pda,
            system_program: solana_sdk::system_program::ID,
            // for emit cpi
            event_authority,
            program: program_id,
        }
        .to_account_metas(None),
        data: solana_axelar_its::instruction::SetTokenManagerFlowLimit { flow_limit }.data(),
    };

    let updated_token_manager_account = add_flow_limiter_result
        .get_account(&token_manager_pda)
        .unwrap();

    let accounts = vec![
        (payer, payer_account),
        (malicious_flow_limiter, flow_limiter_user_account),
        (its_root_pda, its_root_account),
        (token_manager_pda, updated_token_manager_account.clone()),
        (flow_limiter_roles_pda, flow_limiter_roles_account.clone()),
        keyed_account_for_system_program(),
        // for event cpi
        (event_authority, event_authority_account),
        (program_id, program_account),
    ];

    let checks = vec![Check::err(
        anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::ConstraintSeeds).into(),
    )];

    mollusk.process_and_validate_instruction(&set_token_manager_flow_limit_ix, &accounts, &checks);
}

#[test]
fn reject_set_token_manager_flow_limit_without_operator_role() {
    let program_id = solana_axelar_its::id();
    let mollusk = initialize_mollusk_with_programs();

    let (payer, payer_account) = new_test_account();
    let (deployer, deployer_account) = new_test_account();
    let (operator, operator_account) = new_test_account();

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

    // Deploy an interchain token first
    let salt = [1u8; 32];
    let name = "Test Token".to_owned();
    let symbol = "TEST".to_owned();
    let decimals = 9u8;
    let initial_supply = 1_000_000_000u64; // 1 billion tokens with 9 decimals

    let minter = Pubkey::new_unique();

    let token_id = interchain_token_id(&deployer, &salt);
    let (token_manager_pda, _) = TokenManager::find_pda(token_id, its_root_pda);
    let (minter_roles_pda, _) = UserRoles::find_pda(&token_manager_pda, &minter);

    let ctx = DeployInterchainTokenContext::new(
        mollusk,
        (its_root_pda, its_root_account.clone()),
        (deployer, deployer_account),
        (payer, payer_account.clone()),
        Some(minter),
        Some(minter_roles_pda),
    );

    let (result, token_manager_pda, _, _, _, _, mollusk) = deploy_interchain_token_helper(
        ctx,
        salt,
        name.clone(),
        symbol.clone(),
        decimals,
        initial_supply,
        vec![Check::success()],
    );

    assert!(result.program_result.is_ok());

    // Verify the token manager was created with no flow limit initially
    let token_manager_account = result.get_account(&token_manager_pda).unwrap();
    let token_manager =
        TokenManager::try_deserialize(&mut token_manager_account.data.as_ref()).unwrap();

    assert_eq!(token_manager.flow_slot.flow_limit, None);

    // Add a dedicated flow limiter user (different from minter/operator)
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

    let minter_roles_account = result
        .get_account(&minter_roles_pda)
        .expect("Minter roles account should exist after token deployment");

    let minter_account = new_default_account();

    let add_flow_limiter_accounts = vec![
        keyed_account_for_system_program(),
        (payer, payer_account.clone()),
        (minter, minter_account),
        (minter_roles_pda, minter_roles_account.clone()),
        (its_root_pda, its_root_account.clone()),
        (
            token_manager_pda,
            result.get_account(&token_manager_pda).unwrap().clone(),
        ),
        (flow_limiter_user, flow_limiter_user_account.clone()),
        (flow_limiter_roles_pda, new_empty_account()),
    ];

    let add_flow_limiter_result =
        mollusk.process_instruction(&add_flow_limiter_ix, &add_flow_limiter_accounts);

    assert!(add_flow_limiter_result.program_result.is_ok());

    // Verify that the flow limiter role was added correctly
    let flow_limiter_roles_account = add_flow_limiter_result
        .get_account(&flow_limiter_roles_pda)
        .expect("Flow limiter roles account should exist after adding flow limiter");

    // Remove roles from minter account
    let mut flow_limiter_roles_account_clone = flow_limiter_roles_account.clone();

    let mut flow_limiter_roles =
        UserRoles::try_deserialize(&mut flow_limiter_roles_account_clone.data.as_ref())
            .expect("Failed to deserialize flow limiter roles");
    flow_limiter_roles.roles = roles::EMPTY;

    let mut new_data = Vec::new();
    new_data.extend_from_slice(UserRoles::DISCRIMINATOR);
    flow_limiter_roles
        .serialize(&mut new_data)
        .expect("Failed to serialize");
    flow_limiter_roles_account_clone.data = new_data;

    // Now set the token manager flow limit using the dedicated flow limiter
    let flow_limit: Option<u64> = Some(1_000_000_000); // 1 billion tokens

    let (event_authority, event_authority_account, program_account) =
        get_event_authority_and_program_accounts(&program_id);

    let set_token_manager_flow_limit_ix = Instruction {
        program_id,
        accounts: solana_axelar_its::accounts::SetTokenManagerFlowLimit {
            payer,
            flow_limiter: flow_limiter_user,
            its_root_pda,
            token_manager_pda,
            flow_limiter_roles_pda,
            system_program: solana_sdk::system_program::ID,
            // for emit cpi
            event_authority,
            program: program_id,
        }
        .to_account_metas(None),
        data: solana_axelar_its::instruction::SetTokenManagerFlowLimit { flow_limit }.data(),
    };

    let updated_token_manager_account = add_flow_limiter_result
        .get_account(&token_manager_pda)
        .unwrap();

    let accounts = vec![
        (payer, payer_account),
        (flow_limiter_user, flow_limiter_user_account),
        (its_root_pda, its_root_account),
        (token_manager_pda, updated_token_manager_account.clone()),
        (
            flow_limiter_roles_pda,
            flow_limiter_roles_account_clone.clone(),
        ),
        keyed_account_for_system_program(),
        // for event cpi
        (event_authority, event_authority_account),
        (program_id, program_account),
    ];

    let checks = vec![Check::err(
        anchor_lang::error::Error::from(RolesError::MissingFlowLimiterRole).into(),
    )];

    mollusk.process_and_validate_instruction(&set_token_manager_flow_limit_ix, &accounts, &checks);
}
