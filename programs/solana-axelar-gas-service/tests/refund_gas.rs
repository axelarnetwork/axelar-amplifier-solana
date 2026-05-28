#![cfg(test)]

use mollusk_harness::{GasServiceSetup, GasServiceTestHarness, TestHarness};
use mollusk_svm::result::Check;
use solana_sdk::pubkey::Pubkey;

#[test]
fn refund_native_fees() {
    let mut harness = GasServiceTestHarness::new();

    let treasury_funding = 10_000_000_000;
    harness.fund_treasury(treasury_funding);

    let receiver = Pubkey::new_unique();
    let receiver_balance = 1_000_000_000;
    let amount = 500_000_000;
    harness.ensure_account_exists_with_lamports(receiver, receiver_balance);

    let treasury_balance = harness
        .get_account(&harness.treasury())
        .expect("treasury should exist")
        .lamports;

    let ix = harness.refund_fees_ix(receiver, "tx-sig-2.1".to_owned(), amount);

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
