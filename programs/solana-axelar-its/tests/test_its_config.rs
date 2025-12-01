#![cfg(test)]
#![allow(clippy::indexing_slicing)]
//! TEMPORARY: Test file using the new test harness. The tests will be split up into multiple files as the harness gets adopted.

use mollusk_harness::{ItsTestHarness, TestHarness};
#[allow(unused)]
use mollusk_svm::result::Check;

use solana_axelar_its::instructions::{
    make_initialize_instruction, make_set_pause_status_instruction,
};
use solana_axelar_its::{ItsError, Roles, UserRoles};

#[test]
fn test_init_unauthorized_payer() {
    let mut its_harness = ItsTestHarness::default();

    let upgrade_authority = its_harness.get_new_wallet();
    let payer = its_harness.get_new_wallet();
    let operator = its_harness.get_new_wallet();

    its_harness.ensure_program_data_account(
        "solana_axelar_its",
        &solana_axelar_its::ID,
        upgrade_authority,
    );

    let (init_ix, _init_accounts) = make_initialize_instruction(
        payer, // doesn't match upgrade authority
        operator,
        "solana".to_owned(),
        "axelar123".to_owned(),
    );

    its_harness.ctx.process_and_validate_instruction(
        &init_ix,
        &[Check::err(ItsError::InvalidAccountData.into())],
    );
}

#[test]
fn test_init_gives_user_role_to_operator() {
    let its_harness = ItsTestHarness::new();

    let user_roles_pda = UserRoles::find_pda(&its_harness.its_root, &its_harness.operator).0;
    let user_roles: UserRoles = its_harness
        .get_account_as(&user_roles_pda)
        .expect("user roles account should exist");

    assert_eq!(
        user_roles.roles,
        Roles::OPERATOR,
        "user should be an operator"
    );
}

//
// Pause/unpause
//

#[test]
fn test_set_pause_status() {
    let harness = ItsTestHarness::new();

    // Verify initial state is unpaused
    assert!(!harness.get_its_root().paused);

    // Pause the service
    let pause_ix = make_set_pause_status_instruction(harness.operator, true).0;

    harness
        .ctx
        .process_and_validate_instruction(&pause_ix, &[Check::success()]);

    // Verify paused
    assert!(harness.get_its_root().paused);

    // Unpause the service
    let unpause_ix = make_set_pause_status_instruction(harness.operator, false).0;

    harness
        .ctx
        .process_and_validate_instruction(&unpause_ix, &[Check::success()]);

    // Verify unpaused
    assert!(!harness.get_its_root().paused);
}

#[test]
fn test_set_pause_status_already_paused() {
    let harness = ItsTestHarness::new();

    // Pause first
    let pause_ix = make_set_pause_status_instruction(harness.operator, true).0;
    harness
        .ctx
        .process_and_validate_instruction(&pause_ix, &[Check::success()]);

    // Try to pause again (should fail)
    let duplicate_pause_ix = make_set_pause_status_instruction(harness.operator, true).0;

    harness.ctx.process_and_validate_instruction(
        &duplicate_pause_ix,
        &[Check::err(
            solana_axelar_its::ItsError::InvalidArgument.into(),
        )],
    );
}

#[test]
fn test_set_pause_status_unauthorized() {
    let harness = ItsTestHarness::new();

    // Create unauthorized user
    let unauthorized_payer = harness.get_new_wallet();

    // Try to pause with unauthorized user
    let ix = make_set_pause_status_instruction(unauthorized_payer, true).0;

    harness
        .ctx
        .process_and_validate_instruction(&ix, &[Check::err(ItsError::InvalidAccountData.into())]);

    // Verify the pause status was NOT changed
    assert!(!harness.get_its_root().paused);
}

#[test]
fn test_set_pause_status_already_unpaused() {
    let harness = ItsTestHarness::new();

    // Try to unpause when already unpaused (should fail)
    let ix = make_set_pause_status_instruction(harness.operator, false).0;

    harness
        .ctx
        .process_and_validate_instruction(&ix, &[Check::err(ItsError::InvalidArgument.into())]);
}
