#![cfg(test)]

use mollusk_harness::{ItsTestHarness, TestHarness};
use mollusk_svm::result::Check;
use solana_axelar_its::instructions::make_register_token_metadata_instruction;
use solana_axelar_its::ItsError;

#[test]
fn register_token_metadata() {
    let harness = ItsTestHarness::new();

    // Create a native SPL token (not an interchain token)
    let mint_authority = harness.get_new_wallet();
    let token_mint = harness.create_spl_token_mint(mint_authority, 9, None);

    let (ix, _) = make_register_token_metadata_instruction(
        harness.payer,
        token_mint,
        0, // gas_value
    );

    harness
        .ctx
        .process_and_validate_instruction(&ix, &[Check::success()]);
}

#[test]
fn register_token_metadata_with_gas() {
    let harness = ItsTestHarness::new();

    let mint_authority = harness.get_new_wallet();
    let token_mint = harness.create_spl_token_mint(mint_authority, 9, None);

    let (ix, _) = make_register_token_metadata_instruction(
        harness.payer,
        token_mint,
        10_000, // gas_value
    );

    harness
        .ctx
        .process_and_validate_instruction(&ix, &[Check::success()]);
}

#[test]
fn register_token_metadata_different_decimals() {
    let harness = ItsTestHarness::new();

    let mint_authority = harness.get_new_wallet();

    // Test with 6 decimals (like USDC)
    let token_mint = harness.create_spl_token_mint(mint_authority, 6, None);

    let (ix, _) = make_register_token_metadata_instruction(harness.payer, token_mint, 0);

    harness
        .ctx
        .process_and_validate_instruction(&ix, &[Check::success()]);
}

#[test]
fn register_token_metadata_paused_fails() {
    let harness = ItsTestHarness::new();

    let mint_authority = harness.get_new_wallet();
    let token_mint = harness.create_spl_token_mint(mint_authority, 9, None);

    // Pause ITS
    let pause_ix =
        solana_axelar_its::instructions::make_set_pause_status_instruction(harness.operator, true)
            .0;
    harness
        .ctx
        .process_and_validate_instruction(&pause_ix, &[Check::success()]);

    let (ix, _) = make_register_token_metadata_instruction(harness.payer, token_mint, 0);

    harness
        .ctx
        .process_and_validate_instruction(&ix, &[Check::err(ItsError::Paused.into())]);
}

#[test]
fn register_token_metadata_multiple_times() {
    let harness = ItsTestHarness::new();

    let mint_authority = harness.get_new_wallet();
    let token_mint = harness.create_spl_token_mint(mint_authority, 9, None);

    // Register once
    let (ix1, _) = make_register_token_metadata_instruction(harness.payer, token_mint, 0);
    harness
        .ctx
        .process_and_validate_instruction(&ix1, &[Check::success()]);

    // Register again (should succeed - no restriction on re-registering)
    let (ix2, _) = make_register_token_metadata_instruction(harness.payer, token_mint, 0);
    harness
        .ctx
        .process_and_validate_instruction(&ix2, &[Check::success()]);
}

#[test]
fn register_token_metadata_different_tokens() {
    let harness = ItsTestHarness::new();

    let mint_authority = harness.get_new_wallet();

    // Create two different tokens with different decimals
    let token_mint_1 = harness.create_spl_token_mint(mint_authority, 9, None);
    let token_mint_2 = harness.create_spl_token_mint(mint_authority, 6, None);

    // Register metadata for both tokens
    let (ix1, _) = make_register_token_metadata_instruction(harness.payer, token_mint_1, 0);
    harness
        .ctx
        .process_and_validate_instruction(&ix1, &[Check::success()]);

    let (ix2, _) = make_register_token_metadata_instruction(harness.payer, token_mint_2, 0);
    harness
        .ctx
        .process_and_validate_instruction(&ix2, &[Check::success()]);
}
