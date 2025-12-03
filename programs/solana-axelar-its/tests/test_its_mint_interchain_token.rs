#![cfg(test)]

use anchor_spl::associated_token::get_associated_token_address_with_program_id;
use anchor_spl::token_2022::spl_token_2022;
use mollusk_harness::{ItsTestHarness, TestHarness};
use mollusk_svm::result::Check;
use solana_axelar_its::instructions::make_mint_interchain_token_instruction;
use solana_axelar_its::{ItsError, Roles, RolesError, UserRoles};

#[test]
fn mint_interchain_tokens() {
    let its_harness = ItsTestHarness::new();

    // Token
    let token_id = its_harness.ensure_test_interchain_token();
    let token_mint =
        solana_axelar_its::TokenManager::find_token_mint(token_id, its_harness.its_root).0;
    let mint_amount = 1_000_000u64;

    // Receiver
    let receiver = its_harness.get_new_wallet();
    let (receiver_ata, _) =
        its_harness.get_or_create_ata_2022_account(its_harness.payer, receiver, token_mint);

    // Mint
    its_harness.ensure_mint_test_interchain_token(token_id, mint_amount, receiver_ata);
}

#[test]
fn mint_interchain_token_zero_amount_fails() {
    let harness = ItsTestHarness::new();

    let token_id = harness.ensure_test_interchain_token();
    let token_mint = harness.token_mint_for_id(token_id);

    let destination = harness.get_new_wallet();
    let (destination_ata, _) =
        harness.get_or_create_ata_2022_account(harness.payer, destination, token_mint);

    let (mint_ix, _) = make_mint_interchain_token_instruction(
        token_id,
        0, // zero amount
        harness.operator,
        destination_ata,
        spl_token_2022::ID,
    );

    harness
        .ctx
        .process_and_validate_instruction(&mint_ix, &[Check::err(ItsError::InvalidAmount.into())]);
}

#[test]
fn mint_interchain_token_unauthorized_minter_fails() {
    let harness = ItsTestHarness::new();

    let token_id = harness.ensure_test_interchain_token();
    let token_mint = harness.token_mint_for_id(token_id);

    let destination = harness.get_new_wallet();
    let (destination_ata, _) =
        harness.get_or_create_ata_2022_account(harness.payer, destination, token_mint);

    // Use unauthorized minter
    let unauthorized_minter = harness.get_new_wallet();

    let (mint_ix, _) = make_mint_interchain_token_instruction(
        token_id,
        50_000_000,
        unauthorized_minter,
        destination_ata,
        spl_token_2022::ID,
    );

    // Should fail because minter_roles_pda derived from unauthorized_minter doesn't exist
    harness.ctx.process_and_validate_instruction(
        &mint_ix,
        &[Check::err(
            anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::AccountNotInitialized)
                .into(),
        )],
    );
}

#[test]
fn mint_interchain_token_no_minter_role_fails() {
    let mut harness = ItsTestHarness::new();

    let token_id = harness.ensure_test_interchain_token();
    let token_mint = harness.token_mint_for_id(token_id);

    let token_manager_pda = solana_axelar_its::TokenManager::find_pda(token_id, harness.its_root).0;
    let minter_roles_pda = UserRoles::find_pda(&token_manager_pda, &harness.operator).0;

    // Remove minter role from operator
    harness.update_account_as::<UserRoles, _>(&minter_roles_pda, |ur| {
        ur.roles.remove(Roles::MINTER);
    });

    let destination = harness.get_new_wallet();
    let (destination_ata, _) =
        harness.get_or_create_ata_2022_account(harness.payer, destination, token_mint);

    let (mint_ix, _) = make_mint_interchain_token_instruction(
        token_id,
        50_000_000,
        harness.operator,
        destination_ata,
        spl_token_2022::ID,
    );

    harness.ctx.process_and_validate_instruction(
        &mint_ix,
        &[Check::err(RolesError::MissingMinterRole.into())],
    );
}

#[test]
fn mint_interchain_token_paused_fails() {
    let harness = ItsTestHarness::new();

    let token_id = harness.ensure_test_interchain_token();
    let token_mint = harness.token_mint_for_id(token_id);

    // Pause ITS
    let pause_ix =
        solana_axelar_its::instructions::make_set_pause_status_instruction(harness.operator, true)
            .0;
    harness
        .ctx
        .process_and_validate_instruction(&pause_ix, &[Check::success()]);

    let destination = harness.get_new_wallet();
    let (destination_ata, _) =
        harness.get_or_create_ata_2022_account(harness.payer, destination, token_mint);

    let (mint_ix, _) = make_mint_interchain_token_instruction(
        token_id,
        50_000_000,
        harness.operator,
        destination_ata,
        spl_token_2022::ID,
    );

    harness
        .ctx
        .process_and_validate_instruction(&mint_ix, &[Check::err(ItsError::Paused.into())]);
}

#[test]
fn mint_interchain_token_wrong_token_id_fails() {
    let harness = ItsTestHarness::new();

    let _token_id = harness.ensure_test_interchain_token();

    // Use wrong token_id
    let wrong_token_id = [99u8; 32];
    let wrong_token_mint = harness.token_mint_for_id(wrong_token_id);

    let destination = harness.get_new_wallet();
    // This will fail to create ATA since the mint doesn't exist,
    // so we just compute the address
    let destination_ata = get_associated_token_address_with_program_id(
        &destination,
        &wrong_token_mint,
        &spl_token_2022::ID,
    );

    let (mint_ix, _) = make_mint_interchain_token_instruction(
        wrong_token_id,
        50_000_000,
        harness.operator,
        destination_ata,
        spl_token_2022::ID,
    );

    // Should fail because token manager doesn't exist for wrong token_id
    harness.ctx.process_and_validate_instruction(
        &mint_ix,
        &[Check::err(
            anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::AccountNotInitialized)
                .into(),
        )],
    );
}

#[test]
fn mint_interchain_token_multiple_mints() {
    let harness = ItsTestHarness::new();

    let token_id = harness.ensure_test_interchain_token();
    let token_mint = harness.token_mint_for_id(token_id);

    let destination = harness.get_new_wallet();
    let (destination_ata, _) =
        harness.get_or_create_ata_2022_account(harness.payer, destination, token_mint);

    // Mint multiple times
    harness.ensure_mint_test_interchain_token(token_id, 100_000, destination_ata);
    harness.ensure_mint_test_interchain_token(token_id, 200_000, destination_ata);
    harness.ensure_mint_test_interchain_token(token_id, 300_000, destination_ata);

    // Verify total balance
    let ata_data = harness.get_ata_2022_data(destination, token_mint);
    assert_eq!(ata_data.amount, 600_000);
}
