#![cfg(test)]
#![allow(clippy::indexing_slicing)]

pub use anchor_lang::error::{Error, ErrorCode};
use anchor_lang::ToAccountMetas;
use mollusk_harness::{ItsTestHarness, TestHarness};
use mollusk_svm::result::Check;

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
fn test_set_multiple_trusted_chain_by_upgrade_authority() {
    let mut its_harness = ItsTestHarness::new();

    let new_chain_name = "ethereum".to_owned();
    let new_chain_name2 = "arbitrum".to_owned();

    its_harness.ensure_trusted_chain(&new_chain_name);
    its_harness.ensure_trusted_chain(&new_chain_name2);

    let its_root = its_harness.get_its_root();

    assert!(
        its_root.trusted_chains.contains(&new_chain_name),
        "trusted chains should contain the new chain"
    );
    assert!(
        its_root.trusted_chains.contains(&new_chain_name2),
        "trusted chains should contain the second new chain"
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
fn test_set_trusted_chain_unauthorized() {
    let its_harness = ItsTestHarness::new();

    let new_chain_name = "ethereum".to_owned();
    let unauthorized_user = its_harness.get_new_wallet();

    let ix = solana_axelar_its::instructions::make_set_trusted_chain_instruction(
        unauthorized_user,
        new_chain_name.to_owned(),
        false,
    )
    .0;

    its_harness.ctx.process_and_validate_instruction(
        &ix,
        &[Check::err(
            solana_axelar_its::ItsError::InvalidAccountData.into(),
        )],
    );
}

#[test]
fn test_set_trusted_chain_non_operator_user_roles() {
    let mut its_harness = ItsTestHarness::new();

    let chain_name = "ethereum".to_owned();

    let (ix, accounts) = solana_axelar_its::instructions::make_set_trusted_chain_instruction(
        its_harness.operator,
        chain_name.clone(),
        true,
    );

    let user_roles_pubkey = accounts.user_roles.expect("should use user roles auth");

    its_harness.update_account::<solana_axelar_its::UserRoles, _>(&user_roles_pubkey, |roles| {
        roles.roles = solana_axelar_its::Roles::FLOW_LIMITER;
    });

    its_harness.ctx.process_and_validate_instruction(
        &ix,
        &[Check::err(
            solana_axelar_its::RolesError::MissingOperatorRole.into(),
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
    )
    .0;

    its_harness.ctx.process_and_validate_instruction(
        &ix,
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
    )
    .0;

    its_harness.ctx.process_and_validate_instruction(
        &ix,
        &[Check::err(
            solana_axelar_its::ItsError::TrustedChainNotSet.into(),
        )],
    );
}

#[test]
fn test_remove_trusted_chain_invalid_auth() {
    let mut its_harness = ItsTestHarness::new();

    let chain_name = "ethereum".to_owned();
    its_harness.ensure_trusted_chain(&chain_name);

    let (mut ix, mut accounts) =
        solana_axelar_its::instructions::make_remove_trusted_chain_instruction(
            its_harness.operator,
            chain_name.clone(),
            false,
        );

    // Remove both authorization methods
    accounts.user_roles = None;
    accounts.program_data = None;
    ix.accounts = accounts.to_account_metas(Some(true));

    its_harness.ctx.process_and_validate_instruction(
        &ix,
        &[Check::err(
            solana_axelar_its::ItsError::MissingRequiredSignature.into(),
        )],
    );
}

#[test]
fn test_remove_trusted_chain_invalid_user_roles() {
    let mut its_harness = ItsTestHarness::new();

    let chain_name = "ethereum".to_owned();
    its_harness.ensure_trusted_chain(&chain_name);

    let payer = its_harness.get_new_wallet();

    let (mut ix, mut accounts) =
        solana_axelar_its::instructions::make_remove_trusted_chain_instruction(
            payer, // Use as operator here
            chain_name.clone(),
            true,
        );

    // Add user_roles of a valid operator account, but not the payer
    accounts.user_roles = Some(
        solana_axelar_its::UserRoles::find_pda(&its_harness.its_root, &its_harness.operator).0,
    );
    ix.accounts = accounts.to_account_metas(Some(true));

    its_harness.ctx.process_and_validate_instruction(
        &ix,
        &[Check::err(Error::from(ErrorCode::ConstraintSeeds).into())],
    );
}

#[test]
fn test_remove_trusted_chain_non_operator_user_roles() {
    let mut its_harness = ItsTestHarness::new();

    let chain_name = "ethereum".to_owned();
    its_harness.ensure_trusted_chain(&chain_name);

    let (ix, accounts) = solana_axelar_its::instructions::make_remove_trusted_chain_instruction(
        its_harness.operator,
        chain_name.clone(),
        true,
    );

    let user_roles_pubkey = accounts.user_roles.expect("should use user roles auth");

    its_harness.update_account::<solana_axelar_its::UserRoles, _>(&user_roles_pubkey, |roles| {
        roles.roles = solana_axelar_its::Roles::FLOW_LIMITER;
    });

    its_harness.ctx.process_and_validate_instruction(
        &ix,
        &[Check::err(
            solana_axelar_its::RolesError::MissingOperatorRole.into(),
        )],
    );
}
