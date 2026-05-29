#![cfg(test)]
#![allow(clippy::str_to_string)]

use anchor_lang::prelude::ProgramError;
use mollusk_harness::{GasServiceSetup, GasServiceTestHarness, TestHarness};
use mollusk_svm::result::Check;
use solana_sdk::pubkey::Pubkey;

#[test]
fn pay_native_contract_call() {
    let harness = GasServiceTestHarness::new();

    let payer = Pubkey::new_unique();
    let payer_balance = 1_000_000_000;
    let amount = 300_000_000;
    harness.ensure_account_exists_with_lamports(payer, payer_balance);

    let treasury_balance = harness
        .get_account(&harness.treasury())
        .expect("treasury should exist")
        .lamports;

    let ix = harness.pay_gas_ix(
        payer,
        "chain".to_string(),
        "address".to_string(),
        [0; 32],
        amount,
        Pubkey::new_unique(),
    );

    harness.ctx.process_and_validate_instruction(
        &ix,
        &[
            Check::success(),
            Check::account(&payer)
                .lamports(payer_balance - amount)
                .build(),
            Check::account(&harness.treasury())
                .lamports(treasury_balance + amount)
                .build(),
        ],
    );
}

#[test]
fn pay_native_contract_call_fails_for_zero() {
    let harness = GasServiceTestHarness::new();

    let payer = Pubkey::new_unique();
    let payer_balance = 1_000_000_000;
    harness.ensure_account_exists_with_lamports(payer, payer_balance);

    let treasury_balance = harness
        .get_account(&harness.treasury())
        .expect("treasury should exist")
        .lamports;

    let ix = harness.pay_gas_ix(
        payer,
        "chain".to_string(),
        "address".to_string(),
        [0; 32],
        0,
        Pubkey::new_unique(),
    );

    harness.ctx.process_and_validate_instruction(
        &ix,
        &[
            Check::err(ProgramError::InvalidInstructionData),
            Check::account(&payer).lamports(payer_balance).build(),
            Check::account(&harness.treasury())
                .lamports(treasury_balance)
                .build(),
        ],
    );
}

#[test]
fn pay_native_contract_call_fails_insufficient_balance() {
    let harness = GasServiceTestHarness::new();

    let payer = Pubkey::new_unique();
    let payer_balance = 300_000_000;
    let amount = 500_000_000;
    harness.ensure_account_exists_with_lamports(payer, payer_balance);

    let treasury_balance = harness
        .get_account(&harness.treasury())
        .expect("treasury should exist")
        .lamports;

    let ix = harness.pay_gas_ix(
        payer,
        "chain".to_string(),
        "address".to_string(),
        [0; 32],
        amount,
        Pubkey::new_unique(),
    );

    harness.ctx.process_and_validate_instruction(
        &ix,
        &[
            Check::err(ProgramError::Custom(1)),
            Check::account(&payer).lamports(payer_balance).build(),
            Check::account(&harness.treasury())
                .lamports(treasury_balance)
                .build(),
        ],
    );
}
