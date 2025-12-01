#![cfg(test)]
#![allow(clippy::indexing_slicing)]

use mollusk_harness::{ItsTestHarness, TestHarness};
use mollusk_svm::result::Check;
use solana_axelar_its::instructions::make_deploy_remote_interchain_token_instruction;
use solana_axelar_its::ItsError;

#[test]
fn test_deploy_remote_interchain_token() {
    let mut harness = ItsTestHarness::new();

    let _token_id = harness.ensure_test_interchain_token();
    harness.ensure_trusted_chain("ethereum");

    // Deploy remotely to ethereum (trusted by default)
    let (remote_deploy_ix, _) = make_deploy_remote_interchain_token_instruction(
        harness.payer,
        harness.operator, // deployer used by ensure_test_interchain_token
        ItsTestHarness::TEST_TOKEN_SALT,
        "ethereum".to_owned(),
        10_000, // gas_value
    );

    harness
        .ctx
        .process_and_validate_instruction(&remote_deploy_ix, &[Check::success()]);

    // TODO check CPI events
}

#[test]
fn test_deploy_remote_interchain_token_untrusted_chain_fails() {
    let harness = ItsTestHarness::new();

    let _token_id = harness.ensure_test_interchain_token();

    let (remote_deploy_ix, _) = make_deploy_remote_interchain_token_instruction(
        harness.payer,
        harness.operator,
        ItsTestHarness::TEST_TOKEN_SALT,
        "untrusted-chain".to_owned(),
        0,
    );

    harness.ctx.process_and_validate_instruction(
        &remote_deploy_ix,
        &[Check::err(ItsError::UntrustedDestinationChain.into())],
    );
}

#[test]
fn test_deploy_remote_interchain_token_same_chain_fails() {
    let mut harness = ItsTestHarness::new();

    let _token_id = harness.ensure_test_interchain_token();
    harness.ensure_trusted_chain("solana");

    let (remote_deploy_ix, _) = make_deploy_remote_interchain_token_instruction(
        harness.payer,
        harness.operator,
        ItsTestHarness::TEST_TOKEN_SALT,
        "solana".to_owned(), // same as local chain
        0,
    );

    harness.ctx.process_and_validate_instruction(
        &remote_deploy_ix,
        &[Check::err(ItsError::InvalidDestinationChain.into())],
    );
}

#[test]
fn test_deploy_remote_interchain_token_no_local_token_fails() {
    let harness = ItsTestHarness::new();

    // Don't deploy locally, try to deploy remotely directly
    let (remote_deploy_ix, _) = make_deploy_remote_interchain_token_instruction(
        harness.payer,
        harness.operator,
        ItsTestHarness::TEST_TOKEN_SALT,
        "ethereum".to_owned(),
        0,
    );

    harness.ctx.process_and_validate_instruction(
        &remote_deploy_ix,
        &[Check::err(
            anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::AccountNotInitialized)
                .into(),
        )],
    );
}

#[test]
fn test_deploy_remote_interchain_token_paused_fails() {
    let mut harness = ItsTestHarness::new();

    let _token_id = harness.ensure_test_interchain_token();
    harness.ensure_trusted_chain("ethereum");

    // Pause ITS
    let pause_ix =
        solana_axelar_its::instructions::make_set_pause_status_instruction(harness.operator, true)
            .0;
    harness
        .ctx
        .process_and_validate_instruction(&pause_ix, &[Check::success()]);

    // Try to deploy remotely while paused
    let (remote_deploy_ix, _) = make_deploy_remote_interchain_token_instruction(
        harness.payer,
        harness.operator,
        ItsTestHarness::TEST_TOKEN_SALT,
        "ethereum".to_owned(),
        0,
    );

    harness.ctx.process_and_validate_instruction(
        &remote_deploy_ix,
        &[Check::err(ItsError::Paused.into())],
    );
}

#[test]
fn test_deploy_remote_interchain_token_wrong_deployer_fails() {
    let mut harness = ItsTestHarness::new();

    let _token_id = harness.ensure_test_interchain_token();
    harness.ensure_trusted_chain("ethereum");

    // Use a different deployer than the one who deployed locally
    let wrong_deployer = harness.get_new_wallet();

    let (remote_deploy_ix, _) = make_deploy_remote_interchain_token_instruction(
        harness.payer,
        wrong_deployer, // not the original deployer
        ItsTestHarness::TEST_TOKEN_SALT,
        "ethereum".to_owned(),
        0,
    );

    // Should fail because the derived token_mint PDA won't match
    harness.ctx.process_and_validate_instruction(
        &remote_deploy_ix,
        &[Check::err(
            anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::AccountNotInitialized)
                .into(),
        )],
    );
}

#[test]
fn test_deploy_remote_interchain_token_wrong_salt_fails() {
    let mut harness = ItsTestHarness::new();

    let _token_id = harness.ensure_test_interchain_token();
    harness.ensure_trusted_chain("ethereum");

    // Use a different salt than the one used for local deployment
    let wrong_salt = [99u8; 32];

    let (remote_deploy_ix, _) = make_deploy_remote_interchain_token_instruction(
        harness.payer,
        harness.operator,
        wrong_salt, // not the original salt
        "ethereum".to_owned(),
        0,
    );

    // Should fail because the derived token_mint PDA won't match
    harness.ctx.process_and_validate_instruction(
        &remote_deploy_ix,
        &[Check::err(
            anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::AccountNotInitialized)
                .into(),
        )],
    );
}

#[test]
fn test_deploy_remote_interchain_token_multiple_chains() {
    let mut harness = ItsTestHarness::new();

    let _token_id = harness.ensure_test_interchain_token();
    harness.ensure_trusted_chain("ethereum");
    harness.ensure_trusted_chain("avalanche");

    // Deploy to ethereum
    let (deploy_eth_ix, _) = make_deploy_remote_interchain_token_instruction(
        harness.payer,
        harness.operator,
        ItsTestHarness::TEST_TOKEN_SALT,
        "ethereum".to_owned(),
        0,
    );

    harness
        .ctx
        .process_and_validate_instruction(&deploy_eth_ix, &[Check::success()]);

    // Deploy to avalanche (should also succeed - can deploy to multiple chains)
    let (deploy_avax_ix, _) = make_deploy_remote_interchain_token_instruction(
        harness.payer,
        harness.operator,
        ItsTestHarness::TEST_TOKEN_SALT,
        "avalanche".to_owned(),
        0,
    );

    harness
        .ctx
        .process_and_validate_instruction(&deploy_avax_ix, &[Check::success()]);
}
