#![cfg(test)]
#![allow(clippy::indexing_slicing)]
//! TEMPORARY: Test file using the new test harness. The tests will be split up into multiple files as the harness gets adopted.

use mollusk_harness::{ItsTestHarness, TestHarness};
#[allow(unused)]
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
