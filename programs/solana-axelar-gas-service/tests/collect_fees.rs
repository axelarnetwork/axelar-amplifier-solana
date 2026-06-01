#![cfg(test)]

use anchor_lang::prelude::ProgramError;
use anchor_lang::{InstructionData, ToAccountMetas};
use mollusk_harness::{GasServiceSetup, GasServiceTestHarness, OperatorsSetup, TestHarness};
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
fn collect_native_fees_fails_for_zero() {
    let mut harness = GasServiceTestHarness::new();

    harness.fund_treasury(10_000_000_000);

    let receiver = Pubkey::new_unique();
    let receiver_balance = 1_000_000_000;
    harness.ensure_account_exists_with_lamports(receiver, receiver_balance);

    let treasury_balance = harness
        .get_account(&harness.treasury())
        .expect("treasury should exist")
        .lamports;

    let ix = harness.collect_fees_ix(receiver, 0);

    harness.ctx.process_and_validate_instruction(
        &ix,
        &[
            Check::err(ProgramError::InvalidInstructionData),
            Check::account(&receiver).lamports(receiver_balance).build(),
            Check::account(&harness.treasury())
                .lamports(treasury_balance)
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
fn collect_native_fees_requires_operator() {
    let mut harness = GasServiceTestHarness::new();

    harness.fund_treasury(10_000_000_000);

    let unauthorized_operator = Pubkey::new_unique();
    harness.ensure_account_exists_with_lamports(unauthorized_operator, 1_000_000_000);

    let receiver = Pubkey::new_unique();
    let receiver_balance = 1_000_000_000;
    let amount = 500_000_000;
    harness.ensure_account_exists_with_lamports(receiver, receiver_balance);

    let treasury_balance = harness
        .get_account(&harness.treasury())
        .expect("treasury should exist")
        .lamports;

    let (event_authority, _) = solana_axelar_gas_service::EVENT_AUTHORITY_AND_BUMP;
    let ix = solana_sdk::instruction::Instruction {
        program_id: solana_axelar_gas_service::ID,
        accounts: solana_axelar_gas_service::accounts::CollectFees {
            operator: unauthorized_operator,
            operator_pda: harness.operator_account(&harness.operator),
            receiver,
            treasury: harness.treasury(),
            event_authority,
            program: solana_axelar_gas_service::ID,
        }
        .to_account_metas(None),
        data: solana_axelar_gas_service::instruction::CollectFees { amount }.data(),
    };

    harness.ctx.process_and_validate_instruction(
        &ix,
        &[
            Check::err(ProgramError::Custom(2006)),
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
    let amount = 10_000_000_000
        + initial_treasury_balance
            .checked_div(2)
            .expect("initial treasury balance should divide by two");
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
