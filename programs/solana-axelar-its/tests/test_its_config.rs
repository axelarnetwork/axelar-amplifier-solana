#![cfg(test)]
#![allow(clippy::indexing_slicing)]
//! TEMPORARY: Test file using the new test harness. The tests will be split up into multiple files as the harness gets adopted.

use mollusk_harness::{ItsTestHarness, TestHarness};
#[allow(unused)]
use mollusk_svm::result::Check;

use solana_axelar_its::instructions::{
    make_accept_operatorship_instruction, make_initialize_instruction,
    make_propose_operatorship_instruction, make_set_pause_status_instruction,
};
use solana_axelar_its::{ItsError, RoleProposal, Roles, RolesError, UserRoles};

//
// Initialize
//

#[test]
fn test_init_unauthorized_payer() {
    let mut harness = ItsTestHarness::default();

    let upgrade_authority = harness.get_new_wallet();
    let payer = harness.get_new_wallet();
    let operator = harness.get_new_wallet();

    harness.ensure_program_data_account(
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

    harness.ctx.process_and_validate_instruction(
        &init_ix,
        &[Check::err(ItsError::InvalidAccountData.into())],
    );
}

#[test]
fn test_init_gives_user_role_to_operator() {
    let harness = ItsTestHarness::new();

    let user_roles_pda = UserRoles::find_pda(&harness.its_root, &harness.operator).0;
    let user_roles: UserRoles = harness
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

//
// Transfer operatorship
//

#[test]
fn test_transfer_operatorship() {
    let mut its_harness = ItsTestHarness::new();

    let new_operator = its_harness.get_new_wallet();

    // This will ensure the roles have been transferred
    // and the previous operator's roles PDA has been deleted
    its_harness.ensure_transfer_operatorship(new_operator);
}

#[test]
fn test_transfer_operatorship_without_deleting_roles_pda() {
    let mut its_harness = ItsTestHarness::new();

    let curr_operator = its_harness.operator;
    let curr_roles_pda = UserRoles::find_pda(&its_harness.its_root, &curr_operator).0;

    // Append FLOW_LIMITER role to current operator
    its_harness.update_account_as::<UserRoles, _>(&curr_roles_pda, |ur| {
        ur.roles.insert(Roles::FLOW_LIMITER);
    });

    let new_operator = its_harness.get_new_wallet();
    its_harness.ensure_transfer_operatorship(new_operator);

    let updated_curr_roles: UserRoles = its_harness
        .get_account_as(&curr_roles_pda)
        .expect("current roles account should still exist");

    assert_eq!(
        updated_curr_roles.roles,
        Roles::FLOW_LIMITER,
        "current operator should still have FLOW_LIMITER role"
    );
}

#[test]
fn test_transfer_operatorship_without_permissions() {
    let mut its_harness = ItsTestHarness::new();

    let curr_operator = its_harness.operator;
    let curr_roles_pda = UserRoles::find_pda(&its_harness.its_root, &curr_operator).0;

    // Set only FLOW_LIMITER role to current operator
    its_harness
        .update_account_as::<UserRoles, _>(&curr_roles_pda, |ur| ur.roles = Roles::FLOW_LIMITER);

    let new_operator = its_harness.get_new_wallet();

    let ix = solana_axelar_its::instructions::make_transfer_operatorship_instruction(
        its_harness.payer,
        curr_operator,
        new_operator,
    )
    .0;

    // Process
    its_harness.ctx.process_and_validate_instruction(
        &ix,
        &[Check::err(RolesError::MissingOperatorRole.into())],
    );
}

//
// Propose/Accept operatorship
//

#[test]
fn test_propose_operatorship() {
    let harness = ItsTestHarness::new();

    let proposed_operator = harness.get_new_wallet();

    let ix =
        make_propose_operatorship_instruction(harness.payer, harness.operator, proposed_operator).0;

    let proposal_pda = RoleProposal::find_pda(
        &harness.its_root,
        &harness.operator,
        &proposed_operator,
        &solana_axelar_its::ID,
    )
    .0;

    harness.ctx.process_and_validate_instruction(
        &ix,
        &[
            Check::success(),
            Check::account(&proposal_pda).rent_exempt().build(),
        ],
    );

    // Verify proposal was created
    let proposal: RoleProposal = harness
        .get_account_as(&proposal_pda)
        .expect("proposal account should exist");

    assert_eq!(proposal.roles, Roles::OPERATOR);
}

#[test]
fn test_propose_operatorship_to_self_fails() {
    let harness = ItsTestHarness::new();

    let ix = make_propose_operatorship_instruction(
        harness.payer,
        harness.operator,
        harness.operator, // proposing to self
    )
    .0;

    harness
        .ctx
        .process_and_validate_instruction(&ix, &[Check::err(ItsError::InvalidArgument.into())]);
}

#[test]
fn test_propose_operatorship_without_operator_role_fails() {
    let mut harness = ItsTestHarness::new();

    let proposed_operator = harness.get_new_wallet();

    // Set only FLOW_LIMITER role to current operator
    let curr_roles_pda = UserRoles::find_pda(&harness.its_root, &harness.operator).0;
    harness.update_account_as::<UserRoles, _>(&curr_roles_pda, |ur| ur.roles = Roles::FLOW_LIMITER);

    let ix =
        make_propose_operatorship_instruction(harness.payer, harness.operator, proposed_operator).0;

    harness.ctx.process_and_validate_instruction(
        &ix,
        &[Check::err(RolesError::MissingOperatorRole.into())],
    );
}

#[test]
fn test_accept_operatorship() {
    let harness = ItsTestHarness::new();

    let new_operator = harness.get_new_wallet();

    // First propose
    let propose_ix =
        make_propose_operatorship_instruction(harness.payer, harness.operator, new_operator).0;

    harness
        .ctx
        .process_and_validate_instruction(&propose_ix, &[Check::success()]);

    let old_operator_roles_pda = UserRoles::find_pda(&harness.its_root, &harness.operator).0;
    let proposal_pda = RoleProposal::find_pda(
        &harness.its_root,
        &harness.operator,
        &new_operator,
        &solana_axelar_its::ID,
    )
    .0;

    // Then accept
    let accept_ix =
        make_accept_operatorship_instruction(harness.payer, harness.operator, new_operator).0;

    harness.ctx.process_and_validate_instruction(
        &accept_ix,
        &[
            Check::success(),
            // Verify proposal PDA was closed
            Check::account(&proposal_pda).closed().build(),
            // Verify old operator's roles PDA was closed
            Check::account(&old_operator_roles_pda).closed().build(),
        ],
    );

    // Verify new operator has the role
    let new_operator_roles_pda = UserRoles::find_pda(&harness.its_root, &new_operator).0;
    let new_operator_roles: UserRoles = harness
        .get_account_as(&new_operator_roles_pda)
        .expect("new operator roles account should exist");

    assert!(new_operator_roles.roles.contains(Roles::OPERATOR));
}

#[test]
fn test_accept_operatorship_without_proposal_fails() {
    let harness = ItsTestHarness::new();

    let new_operator = harness.get_new_wallet();

    // Try to accept without proposing first
    let accept_ix =
        make_accept_operatorship_instruction(harness.payer, harness.operator, new_operator).0;

    harness.ctx.process_and_validate_instruction(
        &accept_ix,
        &[Check::err(
            anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::AccountNotInitialized)
                .into(),
        )],
    );
}

#[test]
fn test_accept_operatorship_keeps_other_roles() {
    let mut harness = ItsTestHarness::new();

    let curr_operator = harness.operator;
    let curr_roles_pda = UserRoles::find_pda(&harness.its_root, &curr_operator).0;

    // Add FLOW_LIMITER role to current operator
    harness.update_account_as::<UserRoles, _>(&curr_roles_pda, |ur| {
        ur.roles.insert(Roles::FLOW_LIMITER);
    });

    let new_operator = harness.get_new_wallet();

    // Propose
    let propose_ix =
        make_propose_operatorship_instruction(harness.payer, curr_operator, new_operator).0;
    harness
        .ctx
        .process_and_validate_instruction(&propose_ix, &[Check::success()]);

    // Accept
    let accept_ix =
        make_accept_operatorship_instruction(harness.payer, curr_operator, new_operator).0;
    harness
        .ctx
        .process_and_validate_instruction(&accept_ix, &[Check::success()]);

    // Verify old operator still has FLOW_LIMITER role (account not closed)
    let old_operator_roles: UserRoles = harness
        .get_account_as(&curr_roles_pda)
        .expect("old operator roles account should still exist");

    assert_eq!(
        old_operator_roles.roles,
        Roles::FLOW_LIMITER,
        "old operator should still have FLOW_LIMITER role"
    );
    assert!(
        !old_operator_roles.roles.contains(Roles::OPERATOR),
        "old operator should not have OPERATOR role"
    );
}
