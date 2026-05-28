#![cfg(test)]

use mollusk_harness::{OperatorsSetup, OperatorsTestHarness, TestHarness};
use solana_axelar_operators::OperatorRegistry;
use solana_sdk::pubkey::Pubkey;

#[test]
fn initialize_add_remove() {
    let harness = OperatorsTestHarness::new();

    let operator1 = Pubkey::new_unique();
    harness.add_operator(operator1);

    let operator2 = Pubkey::new_unique();
    harness.add_operator(operator2);

    harness.remove_operator(operator1);

    let registry_state: OperatorRegistry = harness
        .get_account_as(&harness.registry())
        .expect("registry account should deserialize");

    assert_eq!(
        registry_state.operator_count, 1,
        "operator count should be decremented to 1"
    );
}

#[test]
fn transfer_master_works() {
    let mut harness = OperatorsTestHarness::new();

    let new_owner = Pubkey::new_unique();
    harness.transfer_owner(new_owner);

    let registry_state: OperatorRegistry = harness
        .get_account_as(&harness.registry())
        .expect("registry account should deserialize");

    assert_eq!(
        registry_state.owner, new_owner,
        "master operator should be updated to new master"
    );

    let operator = Pubkey::new_unique();
    harness.owner = new_owner;
    harness.add_operator(operator);
}
