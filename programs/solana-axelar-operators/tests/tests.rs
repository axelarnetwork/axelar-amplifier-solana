#![cfg(test)]

use anchor_lang::Discriminator;
use mollusk_harness::{OperatorsSetup, OperatorsTestHarness, TestHarness};
use mollusk_svm::result::Check;
use solana_axelar_operators::ErrorCode;
use solana_axelar_operators::OperatorRegistry;
use solana_sdk::program_error::ProgramError;
use solana_sdk::pubkey::Pubkey;

fn anchor_error(error: ErrorCode) -> ProgramError {
    ProgramError::Custom(error.into())
}

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

#[test]
fn non_owner_cannot_add_operator() {
    let mut harness = OperatorsTestHarness::new();

    let original_owner = harness.owner;
    let unauthorized_owner = Pubkey::new_unique();
    harness.ensure_account_exists_with_lamports(unauthorized_owner, 1_000_000_000);
    harness.owner = unauthorized_owner;

    let operator = Pubkey::new_unique();
    harness.add_operator_with_checks(
        operator,
        &[
            Check::err(anchor_error(ErrorCode::UnauthorizedOwner)),
            Check::account(&harness.registry())
                .data_slice(
                    OperatorRegistry::DISCRIMINATOR.len(),
                    original_owner.as_array(),
                )
                .build(),
        ],
    );
}

#[test]
fn non_owner_cannot_remove_operator() {
    let mut harness = OperatorsTestHarness::new();

    let operator = Pubkey::new_unique();
    harness.add_operator(operator);

    let unauthorized_owner = Pubkey::new_unique();
    harness.ensure_account_exists_with_lamports(unauthorized_owner, 1_000_000_000);
    harness.owner = unauthorized_owner;

    harness.remove_operator_with_checks(
        operator,
        &[
            Check::err(anchor_error(ErrorCode::UnauthorizedOwner)),
            Check::account(&harness.operator_account(&operator))
                .owner(&solana_axelar_operators::ID)
                .build(),
        ],
    );
}

#[test]
fn non_owner_cannot_transfer_owner() {
    let mut harness = OperatorsTestHarness::new();

    let original_owner = harness.owner;
    let unauthorized_owner = Pubkey::new_unique();
    harness.ensure_account_exists_with_lamports(unauthorized_owner, 1_000_000_000);
    harness.owner = unauthorized_owner;

    harness.transfer_owner_with_checks(
        Pubkey::new_unique(),
        &[
            Check::err(anchor_error(ErrorCode::UnauthorizedOwner)),
            Check::account(&harness.registry())
                .data_slice(
                    OperatorRegistry::DISCRIMINATOR.len(),
                    original_owner.as_array(),
                )
                .build(),
        ],
    );
}

#[test]
fn transfer_owner_to_same_owner_fails() {
    let harness = OperatorsTestHarness::new();

    harness.transfer_owner_with_checks(
        harness.owner,
        &[
            Check::err(anchor_error(ErrorCode::SameOwner)),
            Check::account(&harness.registry())
                .data_slice(
                    OperatorRegistry::DISCRIMINATOR.len(),
                    harness.owner.as_array(),
                )
                .build(),
        ],
    );
}
