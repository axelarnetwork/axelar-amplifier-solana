#![cfg(test)]

use anchor_lang::prelude::ProgramError;
use anchor_spl::associated_token::get_associated_token_address_with_program_id;
use anchor_spl::token_2022::spl_token_2022::{self, extension::StateWithExtensions};
use mollusk_harness::{ItsTestHarness, TestHarness};
use mollusk_svm::result::Check;
use solana_axelar_its::instructions::make_register_canonical_token_instruction;
use solana_axelar_its::state::TokenManager;
use solana_sdk::pubkey::Pubkey;
use spl_token_2022::state::Account as Token2022Account;

#[test]
fn test_register_canonical_token() {
    let harness = ItsTestHarness::new();

    // Create a native SPL token mint (not an interchain token)
    let mint_authority = harness.get_new_wallet();
    let token_mint = harness.create_spl_token_mint(mint_authority, 9, Some(1_000_000_000));

    let name = "Canonical Token".to_owned();
    let symbol = "CTKN".to_owned();

    // Create metadata for the token
    harness.create_token_metadata(token_mint, mint_authority, name, symbol);

    // Register it as a canonical token
    // This will check the instruction succeeds
    let token_id = harness.ensure_register_canonical_token(token_mint);

    // Verify token manager was created correctly
    let (token_manager_pda, _) = TokenManager::find_pda(token_id, harness.its_root);
    let token_manager: TokenManager = harness
        .get_account_as(&token_manager_pda)
        .expect("token manager should exist");

    // Verify defaults
    assert_eq!(token_manager.token_id, token_id);
    assert_eq!(token_manager.token_address, token_mint);
    assert_eq!(token_manager.flow_slot.flow_limit, None);
    assert_eq!(token_manager.flow_slot.flow_in, 0);
    assert_eq!(token_manager.flow_slot.flow_out, 0);
    assert_eq!(token_manager.flow_slot.epoch, 0);
    assert_eq!(token_manager.ty, solana_axelar_its::state::Type::LockUnlock);

    // Verify token manager ATA was created
    let token_manager_ata = get_associated_token_address_with_program_id(
        &token_manager_pda,
        &token_mint,
        &spl_token_2022::ID,
    );
    let token_manager_ata_account = harness
        .get_account(&token_manager_ata)
        .expect("token manager ATA should exist");
    let token_manager_ata_data =
        StateWithExtensions::<Token2022Account>::unpack(&token_manager_ata_account.data).unwrap();

    assert_eq!(token_manager_ata_data.base.mint, token_mint);
    assert_eq!(token_manager_ata_data.base.owner, token_manager_pda);
    assert_eq!(token_manager_ata_data.base.amount, 0);
}

#[test]
fn test_reject_register_canonical_token_without_metadata() {
    let harness = ItsTestHarness::new();

    // Create a native SPL token mint without metadata
    let mint_authority = harness.get_new_wallet();
    let token_mint = harness.create_spl_token_mint(mint_authority, 9, Some(1_000_000_000));

    let (ix, _) =
        make_register_canonical_token_instruction(harness.payer, token_mint, spl_token_2022::ID);

    harness.ctx.process_and_validate_instruction(
        &ix,
        &[Check::err(
            solana_axelar_its::ItsError::InvalidAccountData.into(),
        )],
    );
}

#[test]
fn test_reject_register_canonical_token_with_invalid_mint() {
    let harness = ItsTestHarness::new();

    // Use a random pubkey that doesn't have a valid mint account
    let fake_mint = Pubkey::new_unique();
    let mint_authority = harness.get_new_wallet();

    // Create metadata for the fake mint (metadata exists but mint doesn't)
    harness.create_token_metadata(
        fake_mint,
        mint_authority,
        "Fake Token".to_owned(),
        "FAKE".to_owned(),
    );

    let (ix, _) =
        make_register_canonical_token_instruction(harness.payer, fake_mint, spl_token_2022::ID);

    // Should fail because the mint account doesn't exist or is invalid
    harness.ctx.process_and_validate_instruction(
        &ix,
        &[Check::err(
            anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::AccountNotInitialized)
                .into(),
        )],
    );
}

#[test]
fn test_reject_register_same_canonical_token_twice() {
    let harness = ItsTestHarness::new();

    // Create and register a canonical token
    let mint_authority = harness.get_new_wallet();
    let token_mint = harness.create_spl_token_mint(mint_authority, 9, Some(1_000_000_000));

    harness.create_token_metadata(
        token_mint,
        mint_authority,
        "Canonical Token".to_owned(),
        "CTKN".to_owned(),
    );

    // First registration should succeed
    let _token_id = harness.ensure_register_canonical_token(token_mint);

    // Second registration should fail (token manager already exists)
    let (ix, _) =
        make_register_canonical_token_instruction(harness.payer, token_mint, spl_token_2022::ID);

    // The token_manager_pda init constraint should fail
    harness.ctx.process_and_validate_instruction(
        &ix,
        &[Check::err(
            // For some reason the error is a generic Custom(0) instead of AccountAlreadyInitialized
            // The logs being printed are:
            //
            // Allocate: account Address { address: gGcWUDvqWD2KubYySsgF2sf6XHNLCZuDN7b3QgKAbRA, base: None } already in use
            // [2025-12-01T19:08:26.428173000Z DEBUG solana_runtime::message_processor::stable_log]
            // Program 11111111111111111111111111111111 failed: custom program error: 0x0
            ProgramError::Custom(0),
        )],
    );
}

#[test]
fn test_register_canonical_token_different_decimals() {
    let harness = ItsTestHarness::new();

    // Test with different decimal values
    for decimals in [0u8, 6, 9, 18] {
        let mint_authority = harness.get_new_wallet();
        let token_mint = harness.create_spl_token_mint(mint_authority, decimals, Some(1_000_000));

        harness.create_token_metadata(
            token_mint,
            mint_authority,
            format!("Token{decimals}"),
            format!("TK{decimals}"),
        );

        let token_id = harness.ensure_register_canonical_token(token_mint);

        // Verify token manager was created
        let (token_manager_pda, _) = TokenManager::find_pda(token_id, harness.its_root);
        let token_manager: TokenManager = harness
            .get_account_as(&token_manager_pda)
            .expect("token manager should exist");

        assert_eq!(token_manager.token_address, token_mint);
        assert_eq!(token_manager.ty, solana_axelar_its::state::Type::LockUnlock);
    }
}

#[test]
fn test_register_canonical_token_with_transfer_fee() {
    let harness = ItsTestHarness::new();

    // Create a native SPL token mint with transfer fee extension
    let mint_authority = harness.get_new_wallet();
    let transfer_fee_basis_points = 100; // 1%
    let maximum_fee = 1_000_000u64;

    let token_mint = harness.create_spl_token_mint_with_transfer_fee(
        mint_authority,
        9,
        transfer_fee_basis_points,
        maximum_fee,
    );

    harness.create_token_metadata(
        token_mint,
        mint_authority,
        "Fee Token".to_owned(),
        "FEE".to_owned(),
    );

    // Register it as a canonical token
    let token_id = harness.ensure_register_canonical_token(token_mint);

    // Verify token manager was created with LockUnlockFee type
    let (token_manager_pda, _) = TokenManager::find_pda(token_id, harness.its_root);
    let token_manager: TokenManager = harness
        .get_account_as(&token_manager_pda)
        .expect("token manager should exist");

    assert_eq!(token_manager.token_address, token_mint);
    assert_eq!(
        token_manager.ty,
        solana_axelar_its::state::Type::LockUnlockFee
    );
}

#[test]
fn test_reject_register_canonical_token_when_paused() {
    let harness = ItsTestHarness::new();

    // Create a valid canonical token setup
    let mint_authority = harness.get_new_wallet();
    let token_mint = harness.create_spl_token_mint(mint_authority, 9, Some(1_000_000_000));

    harness.create_token_metadata(
        token_mint,
        mint_authority,
        "Paused Token".to_owned(),
        "PAUSE".to_owned(),
    );

    // Pause ITS
    let pause_ix =
        solana_axelar_its::instructions::make_set_pause_status_instruction(harness.operator, true)
            .0;
    harness
        .ctx
        .process_and_validate_instruction(&pause_ix, &[Check::success()]);

    // Try to register canonical token while paused
    let (ix, _) =
        make_register_canonical_token_instruction(harness.payer, token_mint, spl_token_2022::ID);

    harness.ctx.process_and_validate_instruction(
        &ix,
        &[Check::err(solana_axelar_its::ItsError::Paused.into())],
    );
}
