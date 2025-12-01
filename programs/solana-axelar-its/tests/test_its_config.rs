#![cfg(test)]
#![allow(clippy::indexing_slicing)]
//! TEMPORARY: Test file using the new test harness. The tests will be split up into multiple files as the harness gets adopted.

use mollusk_harness::{ItsTestHarness, TestHarness};
use mollusk_svm::result::Check;

#[test]
fn test_init_gives_user_role_to_operator() {
    let its_harness = ItsTestHarness::new();

    let user_roles_pda =
        solana_axelar_its::UserRoles::find_pda(&its_harness.its_root, &its_harness.operator).0;
    let user_roles: solana_axelar_its::UserRoles = its_harness
        .get_account_as(&user_roles_pda)
        .expect("user roles account should exist");

    assert_eq!(
        user_roles.roles,
        solana_axelar_its::Roles::OPERATOR,
        "user should be an operator"
    );
}

#[test]
fn test_set_trusted_chain_by_upgrade_authority() {
    let mut its_harness = ItsTestHarness::new();

    let new_chain_name = "ethereum".to_owned();

    its_harness.ensure_trusted_chain(&new_chain_name);

    let its_root = its_harness.get_its_root();

    assert!(
        its_root.trusted_chains.contains(&new_chain_name),
        "trusted chains should contain the new chain"
    );
}

#[test]
fn test_set_trusted_chain_by_operator() {
    let mut its_harness = ItsTestHarness::new();

    // We transfer the operator to make sure the operator
    // (not the upgrade authority) is performing the action
    let new_operator = its_harness.get_new_wallet();
    its_harness.ensure_transfer_operatorship(new_operator);

    // Now the new operator sets a trusted chain
    let new_chain_name = "ethereum".to_owned();
    let ix = solana_axelar_its::instructions::make_set_trusted_chain_instruction(
        new_operator,
        new_chain_name.to_owned(),
        true, // by_operator
    )
    .0;
    its_harness.ctx.process_and_validate_instruction(
        &ix,
        &[
            Check::success(),
            Check::account(&its_harness.its_root).rent_exempt().build(),
        ],
    );

    // Verify the trusted chain was added
    let its_root = its_harness.get_its_root();
    assert!(
        its_root.trusted_chains.contains(&new_chain_name),
        "trusted chains should contain the new chain"
    );
}

#[test]
fn test_set_trusted_chain_duplicate_fails() {
    let mut its_harness = ItsTestHarness::new();

    let new_chain_name = "ethereum".to_owned();
    its_harness.ensure_trusted_chain(&new_chain_name);

    let ix = solana_axelar_its::instructions::make_set_trusted_chain_instruction(
        its_harness.operator,
        new_chain_name.to_owned(),
        false,
    )
    .0;

    its_harness.ctx.process_and_validate_instruction(
        &ix,
        &[Check::err(
            solana_axelar_its::ItsError::TrustedChainAlreadySet.into(),
        )],
    );
}

#[test]
fn test_remove_trusted_chain_by_upgrade_authority() {
    let mut its_harness = ItsTestHarness::new();

    let new_chain_name = "ethereum".to_owned();
    its_harness.ensure_trusted_chain(&new_chain_name);

    let ix = solana_axelar_its::instructions::make_remove_trusted_chain_instruction(
        its_harness.operator,
        new_chain_name.clone(),
        false,
    );

    its_harness.ctx.process_and_validate_instruction(
        &ix.0,
        &[
            Check::success(),
            Check::account(&its_harness.its_root).rent_exempt().build(),
        ],
    );

    let its_root = its_harness.get_its_root();
    assert!(
        !its_root.trusted_chains.contains(&new_chain_name),
        "trusted chains should not contain the removed chain"
    );
}

#[test]
fn test_remove_unknown_trusted_chain_fails() {
    let its_harness = ItsTestHarness::new();

    let chain_name = "ethereum".to_owned();

    let ix = solana_axelar_its::instructions::make_remove_trusted_chain_instruction(
        its_harness.operator,
        chain_name.clone(),
        false,
    );

    its_harness.ctx.process_and_validate_instruction(
        &ix.0,
        &[Check::err(
            solana_axelar_its::ItsError::TrustedChainNotSet.into(),
        )],
    );
}
