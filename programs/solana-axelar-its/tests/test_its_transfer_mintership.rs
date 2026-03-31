#![cfg(test)]
#![allow(clippy::indexing_slicing)]

use mollusk_harness::{ItsTestHarness, TestHarness};
use mollusk_svm::result::Check;
use solana_axelar_its::{
    instructions::make_transfer_interchain_token_mintership_instruction,
    state::{roles, InterchainTokenService, RolesError, TokenManager, UserRoles},
    ItsError,
};

#[test]
fn transfer_mintership_success() {
    let harness = ItsTestHarness::new();

    // Deploy token with minter
    let token_id = harness.ensure_test_interchain_token();
    let current_minter = harness.operator; // operator is the minter from ensure_test_interchain_token
    let new_minter = harness.get_new_wallet();

    let (ix, _) = make_transfer_interchain_token_mintership_instruction(
        harness.payer,
        current_minter,
        new_minter,
        token_id,
    );

    harness
        .ctx
        .process_and_validate_instruction(&ix, &[Check::success()]);

    // Verify old minter lost MINTER role
    let its_root_pda = InterchainTokenService::find_pda().0;
    let token_manager_pda = TokenManager::find_pda(token_id, its_root_pda).0;

    let old_roles_pda = UserRoles::find_pda(&token_manager_pda, &current_minter).0;
    let old_roles: Option<UserRoles> = harness.get_account_as(&old_roles_pda);
    // Old minter may have had only MINTER role, so account could be closed
    if let Some(r) = old_roles {
        assert!(
            !r.contains(roles::MINTER),
            "old minter should not have MINTER role"
        );
    }

    // Verify new minter has MINTER role
    let new_roles_pda = UserRoles::find_pda(&token_manager_pda, &new_minter).0;
    let new_roles: UserRoles = harness
        .get_account_as(&new_roles_pda)
        .expect("new minter roles should exist");
    assert!(
        new_roles.contains(roles::MINTER),
        "new minter should have MINTER role"
    );
}

#[test]
fn reject_transfer_mintership_unauthorized_minter() {
    let harness = ItsTestHarness::new();

    let token_id = harness.ensure_test_interchain_token();
    let malicious_minter = harness.get_new_wallet();
    let destination = harness.get_new_wallet();

    let (ix, _) = make_transfer_interchain_token_mintership_instruction(
        harness.payer,
        malicious_minter, // not the actual minter
        destination,
        token_id,
    );

    harness.ctx.process_and_validate_instruction(
        &ix,
        &[Check::err(
            anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::AccountNotInitialized)
                .into(),
        )],
    );
}

#[test]
fn reject_transfer_mintership_without_minter_role() {
    let mut harness = ItsTestHarness::new();

    let token_id = harness.ensure_test_interchain_token();
    let its_root_pda = InterchainTokenService::find_pda().0;
    let token_manager_pda = TokenManager::find_pda(token_id, its_root_pda).0;

    // The operator has MINTER role from ensure_test_interchain_token.
    // Strip the MINTER role so only OPERATOR remains.
    let operator_roles_pda = UserRoles::find_pda(&token_manager_pda, &harness.operator).0;
    harness.update_account_as::<UserRoles, _>(&operator_roles_pda, |r| {
        r.remove(roles::MINTER);
    });

    let destination = harness.get_new_wallet();

    let (ix, _) = make_transfer_interchain_token_mintership_instruction(
        harness.payer,
        harness.operator,
        destination,
        token_id,
    );

    harness
        .ctx
        .process_and_validate_instruction(&ix, &[Check::err(RolesError::MissingMinterRole.into())]);
}

#[test]
fn reject_transfer_mintership_same_sender_destination() {
    let harness = ItsTestHarness::new();

    let token_id = harness.ensure_test_interchain_token();
    let current_minter = harness.operator;

    let (ix, _) = make_transfer_interchain_token_mintership_instruction(
        harness.payer,
        current_minter,
        current_minter, // same as sender
        token_id,
    );

    harness
        .ctx
        .process_and_validate_instruction(&ix, &[Check::err(ItsError::InvalidArgument.into())]);
}
