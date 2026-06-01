#![cfg(test)]

use anchor_lang::{InstructionData, ToAccountMetas};
use mollusk_svm::result::Check;
use solana_axelar_mollusk_harness::{
    GasServiceSetup, GasServiceTestHarness, OperatorsSetup, TestHarness,
};
use solana_sdk::pubkey::Pubkey;

#[test]
fn initialize_success() {
    let harness = GasServiceTestHarness::new();

    assert!(harness.account_exists(&harness.treasury()));
    harness.assert_rent_exempt(&harness.treasury());
}

#[test]
#[allow(clippy::should_panic_without_expect)]
#[should_panic]
fn initialize_unauthorized() {
    let harness = GasServiceTestHarness::default();

    harness.ensure_account_exists_with_lamports(harness.payer, 1_000_000_000);
    harness.ensure_account_exists_with_lamports(harness.operator, 1_000_000_000);
    harness.ensure_operators_initialized();
    harness.add_operator(harness.operator);

    let unauthorized_operator = Pubkey::new_unique();
    harness.ensure_account_exists_with_lamports(unauthorized_operator, 1_000_000_000);

    let ix = solana_sdk::instruction::Instruction {
        program_id: solana_axelar_gas_service::ID,
        accounts: solana_axelar_gas_service::accounts::Initialize {
            payer: unauthorized_operator,
            operator: unauthorized_operator,
            operator_pda: harness.operator_account(&harness.operator),
            system_program: solana_sdk_ids::system_program::ID,
            treasury: harness.treasury(),
        }
        .to_account_metas(None),
        data: solana_axelar_gas_service::instruction::Initialize {}.data(),
    };

    harness
        .ctx
        .process_and_validate_instruction(&ix, &[Check::success()]);
}
