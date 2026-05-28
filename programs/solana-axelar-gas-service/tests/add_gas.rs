#![cfg(test)]

use anchor_lang::prelude::ProgramError;
use mollusk_harness::{GasServiceSetup, GasServiceTestHarness, TestHarness};
use mollusk_svm::result::Check;
use solana_sdk::pubkey::Pubkey;

#[test]
fn add_native_gas() {
    let harness = GasServiceTestHarness::new();

    let sender = Pubkey::new_unique();
    let sender_balance = 1_000_000_000;
    let amount = 300_000_000;
    harness.ensure_account_exists_with_lamports(sender, sender_balance);

    let treasury_balance = harness
        .get_account(&harness.treasury())
        .expect("treasury should exist")
        .lamports;

    let ix = harness.add_gas_ix(
        sender,
        "tx-sig-2.1".to_owned(),
        amount,
        Pubkey::new_unique(),
    );

    harness.ctx.process_and_validate_instruction(
        &ix,
        &[
            Check::success(),
            Check::account(&sender)
                .lamports(sender_balance - amount)
                .build(),
            Check::account(&harness.treasury())
                .lamports(treasury_balance + amount)
                .build(),
        ],
    );
}

#[test]
fn add_native_gas_fails_for_zero() {
    let harness = GasServiceTestHarness::new();

    let sender = Pubkey::new_unique();
    let sender_balance = 1_000_000_000;
    harness.ensure_account_exists_with_lamports(sender, sender_balance);

    let treasury_balance = harness
        .get_account(&harness.treasury())
        .expect("treasury should exist")
        .lamports;

    let ix = harness.add_gas_ix(sender, "tx-sig-2.1".to_owned(), 0, Pubkey::new_unique());

    harness.ctx.process_and_validate_instruction(
        &ix,
        &[
            Check::err(ProgramError::InvalidInstructionData),
            Check::account(&sender).lamports(sender_balance).build(),
            Check::account(&harness.treasury())
                .lamports(treasury_balance)
                .build(),
        ],
    );
}

#[test]
fn add_native_gas_fails_insufficient_balance() {
    let harness = GasServiceTestHarness::new();

    let sender = Pubkey::new_unique();
    let sender_balance = 300_000_000;
    let amount = 500_000_000;
    harness.ensure_account_exists_with_lamports(sender, sender_balance);

    let treasury_balance = harness
        .get_account(&harness.treasury())
        .expect("treasury should exist")
        .lamports;

    let ix = harness.add_gas_ix(
        sender,
        "tx-sig-2.1".to_owned(),
        amount,
        Pubkey::new_unique(),
    );

    harness.ctx.process_and_validate_instruction(
        &ix,
        &[
            Check::err(ProgramError::Custom(1)),
            Check::account(&sender).lamports(sender_balance).build(),
            Check::account(&harness.treasury())
                .lamports(treasury_balance)
                .build(),
        ],
    );
}
