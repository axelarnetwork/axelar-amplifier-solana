#![cfg(test)]
#![allow(clippy::too_many_lines)]

use anchor_lang::AccountDeserialize;
use mollusk_svm::result::Check;
use solana_axelar_its::state::{RoleProposal, Roles, TokenManager, UserRoles};
use solana_axelar_its::utils::interchain_token_id;
use solana_axelar_its_test_fixtures::{
    accept_interchain_token_mintership_helper, deploy_interchain_token_helper, init_its_service,
    initialize_mollusk_with_programs, new_empty_account, new_test_account,
    propose_interchain_token_mintership_helper, AcceptInterchainTokenMintershipContext,
    DeployInterchainTokenContext, ProposeInterchainTokenMintershipContext,
};
use solana_sdk::pubkey::Pubkey;

#[test]
fn test_accept_interchain_token_mintership() {
    let program_id = solana_axelar_its::id();
    let mollusk = initialize_mollusk_with_programs();

    let upgrade_authority = Pubkey::new_unique();
    let payer = upgrade_authority;
    let (_, payer_account) = new_test_account();

    let (current_minter, current_minter_account) = new_test_account();
    let (proposed_minter, proposed_minter_account) = new_test_account();

    let chain_name = "solana".to_owned();
    let its_hub_address = "0x123456789abcdef".to_owned();

    let (its_root_pda, its_root_account, _, _, _, _) = init_its_service(
        &mollusk,
        payer,
        &payer_account,
        upgrade_authority,
        current_minter,
        &current_minter_account,
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

    assert!(current_minter_token_roles.roles.contains(Roles::MINTER));

    // Propose mintership transfer
    let (proposal_pda, _bump) = RoleProposal::find_pda(
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

    let (propose_result, mollusk) =
        propose_interchain_token_mintership_helper(ctx, vec![Check::success()]);
    assert!(propose_result.program_result.is_ok());

    // Verify proposal was created
    let proposal_account = propose_result
        .get_account(&proposal_pda)
        .expect("Proposal account should exist");
    let proposal_data = RoleProposal::try_deserialize(&mut proposal_account.data.as_slice())
        .expect("Failed to deserialize RoleProposal");
    assert_eq!(proposal_data.roles, Roles::MINTER);

    // Accept mintership transfer
    let (new_minter_roles_pda, new_minter_roles_bump) =
        UserRoles::find_pda(&token_manager_pda, &proposed_minter);

    let accept_ctx = AcceptInterchainTokenMintershipContext::new(
        mollusk,
        (payer, payer_account.clone()),
        (proposed_minter, proposed_minter_account.clone()),
        new_minter_roles_pda,
        (its_root_pda, its_root_account.clone()),
        (token_manager_pda, token_manager_account.clone()),
        (current_minter, current_minter_account.clone()),
        (
            minter_roles_pda,
            propose_result
                .get_account(&minter_roles_pda)
                .unwrap()
                .clone(),
        ),
        (proposal_pda, proposal_account.clone()),
    );

    let (accept_result, _) =
        accept_interchain_token_mintership_helper(accept_ctx, vec![Check::success()]);
    assert!(accept_result.program_result.is_ok());

    // Old minter should no longer have MINTER role
    let old_minter_roles_account = accept_result
        .get_account(&minter_roles_pda)
        .expect("Old minter roles account should exist");

    let old_minter_roles =
        UserRoles::try_deserialize(&mut old_minter_roles_account.data.as_slice())
            .expect("Failed to deserialize old minter roles");

    assert!(!old_minter_roles.roles.contains(Roles::MINTER));

    // New minter should have MINTER role
    let new_minter_roles_account = accept_result
        .get_account(&new_minter_roles_pda)
        .expect("New minter roles account should exist");
    let new_minter_roles =
        UserRoles::try_deserialize(&mut new_minter_roles_account.data.as_slice())
            .expect("Failed to deserialize new minter roles");

    assert!(new_minter_roles.roles.contains(Roles::MINTER));
    assert_eq!(new_minter_roles.bump, new_minter_roles_bump);

    // Proposal account should be closed
    let proposal_pda_account = accept_result.get_account(&proposal_pda).unwrap();
    assert!(proposal_pda_account.data.is_empty());
}

#[test]
fn test_reject_invalid_interchain_token_mintership() {
    let program_id = solana_axelar_its::id();
    let mollusk = initialize_mollusk_with_programs();

    let upgrade_authority = Pubkey::new_unique();
    let payer = upgrade_authority;
    let (_, payer_account) = new_test_account();

    let (current_minter, current_minter_account) = new_test_account();
    let (proposed_minter, proposed_minter_account) = new_test_account();

    let chain_name = "solana".to_owned();
    let its_hub_address = "0x123456789abcdef".to_owned();

    let (its_root_pda, its_root_account, _, _, _, _) = init_its_service(
        &mollusk,
        payer,
        &payer_account,
        upgrade_authority,
        current_minter,
        &current_minter_account,
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

    assert!(current_minter_token_roles.roles.contains(Roles::MINTER));

    // Propose mintership transfer
    let (proposal_pda, _bump) = RoleProposal::find_pda(
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

    let (propose_result, mollusk) =
        propose_interchain_token_mintership_helper(ctx, vec![Check::success()]);
    assert!(propose_result.program_result.is_ok());

    // Verify proposal was created
    let proposal_account = propose_result
        .get_account(&proposal_pda)
        .expect("Proposal account should exist");
    let proposal_data = RoleProposal::try_deserialize(&mut proposal_account.data.as_slice())
        .expect("Failed to deserialize RoleProposal");
    assert_eq!(proposal_data.roles, Roles::MINTER);

    let (malicious_proposed_minter, malicious_proposed_minter_account) = new_test_account();

    // Attempt to accept mintership transfer with wrong destination user
    let (new_minter_roles_pda, _) =
        UserRoles::find_pda(&token_manager_pda, &malicious_proposed_minter);

    let accept_ctx = AcceptInterchainTokenMintershipContext::new(
        mollusk,
        (payer, payer_account.clone()),
        (malicious_proposed_minter, malicious_proposed_minter_account),
        new_minter_roles_pda,
        (its_root_pda, its_root_account.clone()),
        (token_manager_pda, token_manager_account.clone()),
        (current_minter, current_minter_account.clone()),
        (
            minter_roles_pda,
            propose_result
                .get_account(&minter_roles_pda)
                .unwrap()
                .clone(),
        ),
        (proposal_pda, proposal_account.clone()),
    );

    let checks = vec![Check::err(
        anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::ConstraintSeeds).into(),
    )];

    let (accept_result, _) = accept_interchain_token_mintership_helper(accept_ctx, checks);
    assert!(accept_result.program_result.is_err());

    // Old minter should still have MINTER role
    let old_minter_roles_account = accept_result
        .get_account(&minter_roles_pda)
        .expect("Old minter roles account should exist");

    let old_minter_roles =
        UserRoles::try_deserialize(&mut old_minter_roles_account.data.as_slice())
            .expect("Failed to deserialize old minter roles");

    assert!(old_minter_roles.roles.contains(Roles::MINTER));

    // New minter should not have the MINTER role
    let new_minter_roles_account = accept_result
        .get_account(&new_minter_roles_pda)
        .expect("New minter roles account should exist");

    assert!(new_minter_roles_account.data.is_empty());

    // Proposal account should still be open
    let proposal_pda_account = accept_result.get_account(&proposal_pda).unwrap();
    assert!(!proposal_pda_account.data.is_empty());
}

#[test]
fn test_reject_accept_without_proposal() {
    let mollusk = initialize_mollusk_with_programs();

    let upgrade_authority = Pubkey::new_unique();
    let payer = upgrade_authority;
    let (_, payer_account) = new_test_account();

    let (current_minter, current_minter_account) = new_test_account();
    let (proposed_minter, proposed_minter_account) = new_test_account();

    let chain_name = "solana".to_owned();
    let its_hub_address = "0x123456789abcdef".to_owned();

    let (its_root_pda, its_root_account, _, _, _, _) = init_its_service(
        &mollusk,
        payer,
        &payer_account,
        upgrade_authority,
        current_minter,
        &current_minter_account,
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

    // Attempt to accept mintership transfer without creating a proposal first
    let (proposal_pda, _bump) = RoleProposal::find_pda(
        &token_manager_pda,
        &current_minter,
        &proposed_minter,
        &solana_axelar_its::id(),
    );

    let (new_minter_roles_pda, _) = UserRoles::find_pda(&token_manager_pda, &proposed_minter);

    let accept_ctx = AcceptInterchainTokenMintershipContext::new(
        mollusk,
        (payer, payer_account.clone()),
        (proposed_minter, proposed_minter_account.clone()),
        new_minter_roles_pda,
        (its_root_pda, its_root_account.clone()),
        (token_manager_pda, token_manager_account.clone()),
        (current_minter, current_minter_account.clone()),
        (minter_roles_pda, current_minter_token_roles_account.clone()),
        (proposal_pda, new_empty_account()), // Non-existent proposal
    );

    let checks = vec![Check::err(
        anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::AccountNotInitialized)
            .into(),
    )];

    let (accept_result, _) = accept_interchain_token_mintership_helper(accept_ctx, checks);
    assert!(accept_result.program_result.is_err());
}
