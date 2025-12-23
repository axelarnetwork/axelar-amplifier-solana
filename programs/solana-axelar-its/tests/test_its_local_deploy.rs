#![cfg(test)]
#![allow(clippy::indexing_slicing)]

use mollusk_harness::{ItsTestHarness, TestHarness};
use mollusk_svm::result::Check;

#[test]
fn local_deploy_interchain_token() {
    let its_harness = ItsTestHarness::new();

    let _token_id = its_harness.ensure_test_interchain_token();
}

#[test]
fn local_deploy_zero_supply_token() {
    let its_harness = ItsTestHarness::new();
    let deployer = its_harness.get_new_wallet();

    let (deploy_ix, _deploy_accounts) =
        solana_axelar_its::instructions::make_deploy_interchain_token_instruction(
            its_harness.payer,
            deployer,
            [1; 32],
            "Zero Supply Token".to_owned(),
            "ZST".to_owned(),
            8,
            0,
            None,
        );

    its_harness.ctx.process_and_validate_instruction(
        &deploy_ix,
        &[Check::err(
            solana_axelar_its::ItsError::ZeroSupplyToken.into(),
        )],
    );
}

#[test]
fn local_deploy_initial_supply_token() {
    let its_harness = ItsTestHarness::new();

    let deployer = its_harness.get_new_wallet();
    let initial_supply = 1_000_000u64;

    let (deploy_ix, deploy_accounts) =
        solana_axelar_its::instructions::make_deploy_interchain_token_instruction(
            its_harness.payer,
            deployer,
            [1; 32],
            "Initial Supply Token".to_owned(),
            "IST".to_owned(),
            8,
            initial_supply,
            None,
        );

    its_harness
        .ctx
        .process_and_validate_instruction(&deploy_ix, &[Check::success()]);

    let deployer_ata = deploy_accounts.deployer_ata;
    let token_account = its_harness
        .get_token_account(&deployer_ata)
        .expect("deployer account should exist after deployment");
    assert_eq!(
        token_account.amount, initial_supply,
        "deployer should have the initial supply after deployment"
    );
}

#[test]
fn local_deploy_token_metadata() {
    let its_harness = ItsTestHarness::new();

    let deployer = its_harness.get_new_wallet();

    let name = "Metadata Token".to_owned();
    let symbol = "MDT".to_owned();

    let (deploy_ix, deploy_accounts) =
        solana_axelar_its::instructions::make_deploy_interchain_token_instruction(
            its_harness.payer,
            deployer,
            [1; 32],
            name.clone(),
            symbol.clone(),
            8,
            1_000_000u64,
            None,
        );

    its_harness
        .ctx
        .process_and_validate_instruction(&deploy_ix, &[Check::success()]);

    let metadata_account = deploy_accounts.mpl_token_metadata_account;
    let metadata_account = its_harness
        .get_account(&metadata_account)
        .expect("metadata account should exist after deployment");
    let metadata = mpl_token_metadata::accounts::Metadata::from_bytes(&metadata_account.data)
        .expect("metadata account should deserialize");

    assert!(!metadata.is_mutable, "metadata should be immutable");
    // remove padding
    assert_eq!(metadata.name.trim_matches('\0'), name,);
    assert_eq!(metadata.symbol.trim_matches('\0'), symbol);
    assert_eq!(metadata.mint, deploy_accounts.token_mint);
    assert_eq!(metadata.update_authority, deploy_accounts.token_manager_pda);
}

#[test]
fn local_deploy_long_name_symbol() {
    let its_harness = ItsTestHarness::new();
    let deployer = its_harness.get_new_wallet();

    let (deploy_ix, _deploy_accounts) =
        solana_axelar_its::instructions::make_deploy_interchain_token_instruction(
            its_harness.payer,
            deployer,
            [1; 32],
            "Zero Supply Token".repeat(10).to_owned(),
            "ZST".repeat(15).to_owned(),
            8,
            100,
            None,
        );

    its_harness.ctx.process_and_validate_instruction(
        &deploy_ix,
        &[Check::err(
            solana_axelar_its::ItsError::InvalidArgument.into(),
        )],
    );
}

#[test]
fn local_deploy_minter_roles() {
    use solana_axelar_its::{roles, UserRoles};

    let its_harness = ItsTestHarness::new();
    let deployer = its_harness.get_new_wallet();
    let minter = its_harness.get_new_wallet();

    let (deploy_ix, deploy_accounts) =
        solana_axelar_its::instructions::make_deploy_interchain_token_instruction(
            its_harness.payer,
            deployer,
            [1; 32],
            "Zero Supply Token".to_owned(),
            "ZST".to_owned(),
            8,
            0,
            Some(minter),
        );

    let minter_user_roles = deploy_accounts
        .minter_roles_pda
        .expect("should have minter roles account");

    its_harness.ctx.process_and_validate_instruction(
        &deploy_ix,
        &[
            Check::success(),
            Check::account(&minter_user_roles).rent_exempt().build(),
        ],
    );

    let user_roles: UserRoles = its_harness
        .get_account_as(&minter_user_roles)
        .expect("minter roles account should exist");

    assert_eq!(
        user_roles.roles,
        roles::MINTER | roles::FLOW_LIMITER | roles::OPERATOR,
        "minter should have minter role"
    );
}
