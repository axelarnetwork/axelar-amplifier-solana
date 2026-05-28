#![cfg(test)]

use anchor_lang::prelude::ProgramError;
use mollusk_harness::{GasServiceSetup, GasServiceTestHarness, TestHarness};
use mollusk_svm::result::Check;
use solana_sdk::pubkey::Pubkey;

#[test]
fn collect_native_fees() {
    let mut harness = GasServiceTestHarness::new();

    harness.fund_treasury(10_000_000_000);

    let receiver = Pubkey::new_unique();
    let receiver_balance = 1_000_000_000;
    let amount = 500_000_000;
    harness.ensure_account_exists_with_lamports(receiver, receiver_balance);

    let treasury_balance = harness
        .get_account(&harness.treasury())
        .expect("treasury should exist")
        .lamports;

    let ix = harness.collect_fees_ix(receiver, amount);

    harness.ctx.process_and_validate_instruction(
        &ix,
        &[
            Check::success(),
            Check::account(&receiver)
                .lamports(receiver_balance + amount)
                .build(),
            Check::account(&harness.treasury())
                .lamports(treasury_balance - amount)
                .build(),
        ],
    );
}

#[test]
fn collect_native_fees_insufficient_funds() {
    let mut harness = GasServiceTestHarness::new();

    harness.fund_treasury(10_000_000_000);

    let receiver = Pubkey::new_unique();
    let receiver_balance = 1_000_000_000;
    let amount = 50_000_000_000;
    harness.ensure_account_exists_with_lamports(receiver, receiver_balance);

    let treasury_balance = harness
        .get_account(&harness.treasury())
        .expect("treasury should exist")
        .lamports;

    let ix = harness.collect_fees_ix(receiver, amount);

    harness.ctx.process_and_validate_instruction(
        &ix,
        &[
            Check::err(ProgramError::InsufficientFunds),
            Check::account(&receiver).lamports(receiver_balance).build(),
            Check::account(&harness.treasury())
                .lamports(treasury_balance)
                .build(),
        ],
    );
}

#[test]
fn collect_native_fees_not_rent_exempt() {
    let mut harness = GasServiceTestHarness::new();

    let initial_treasury_balance = harness
        .get_account(&harness.treasury())
        .expect("treasury should exist")
        .lamports;
    harness.fund_treasury(10_000_000_000);

    let receiver = Pubkey::new_unique();
    let receiver_balance = 1_000_000_000;
    let amount = 10_000_000_000 + initial_treasury_balance / 2;
    harness.ensure_account_exists_with_lamports(receiver, receiver_balance);

    let treasury_balance = harness
        .get_account(&harness.treasury())
        .expect("treasury should exist")
        .lamports;

    let ix = harness.collect_fees_ix(receiver, amount);

    harness.ctx.process_and_validate_instruction(
        &ix,
        &[
            Check::err(ProgramError::InvalidAccountData),
            Check::account(&receiver).lamports(receiver_balance).build(),
            Check::account(&harness.treasury())
                .lamports(treasury_balance)
                .rent_exempt()
                .build(),
        ],
    );
}
