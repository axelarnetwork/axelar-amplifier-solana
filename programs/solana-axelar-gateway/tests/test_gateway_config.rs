#![cfg(test)]
#![allow(clippy::indexing_slicing)]

mod helpers;
use helpers::*;

use anchor_lang::{InstructionData, ToAccountMetas};
use mollusk_harness::{GatewayTestHarness, TestHarness};
use mollusk_svm::result::Check;
use solana_axelar_gateway::{GatewayConfig, GatewayError, VerifierSetTracker};
use solana_axelar_std::U256;
use solana_sdk::pubkey::Pubkey;

#[test]
fn initialize_config() {
    let harness = GatewayTestHarness::new();

    let config: GatewayConfig = harness
        .get_account_as(&harness.gateway.root)
        .expect("gateway config should exist");

    assert_eq!(config.current_epoch, U256::from(1u64));
    assert_eq!(config.previous_verifier_set_retention, U256::from(5u64));
    assert_eq!(config.minimum_rotation_delay, 3600);
    assert_eq!(config.last_rotation_timestamp, 0);
    assert_eq!(config.operator, harness.operator);

    let tracker: VerifierSetTracker = harness
        .get_account_as(&harness.gateway.verifier_set_tracker)
        .expect("verifier set tracker should exist");

    assert_eq!(tracker.epoch, U256::from(1u64));
    assert_eq!(
        tracker.verifier_set_hash,
        harness.gateway.verifier_merkle_tree.root().unwrap()
    );
}

#[test]
fn transfer_operatorship() {
    let harness = GatewayTestHarness::new();

    let new_operator = Pubkey::new_unique();
    harness.transfer_gateway_operatorship(new_operator);

    let updated_config: GatewayConfig = harness
        .get_account_as(&harness.gateway.root)
        .expect("gateway config should exist");

    assert_eq!(updated_config.operator, new_operator);
}

#[test]
fn transfer_operatorship_unauthorized() {
    let harness = GatewayTestHarness::new();

    // Build the transfer instruction manually with a random non-operator/non-authority signer
    let unauthorized = Pubkey::new_unique();
    harness.ensure_account_exists_with_lamports(
        unauthorized,
        solana_sdk::native_token::LAMPORTS_PER_SOL,
    );

    let program_data = anchor_lang::prelude::bpf_loader_upgradeable::get_program_data_address(
        &solana_axelar_gateway::ID,
    );

    let (event_authority, _, _) =
        mollusk_test_utils::get_event_authority_and_program_accounts(&solana_axelar_gateway::ID);

    let new_operator = Pubkey::new_unique();

    let ix = solana_sdk::instruction::Instruction {
        program_id: solana_axelar_gateway::ID,
        accounts: solana_axelar_gateway::accounts::TransferOperatorship {
            gateway_root_pda: harness.gateway.root,
            operator_or_upgrade_authority: unauthorized,
            program_data,
            new_operator,
            event_authority,
            program: solana_axelar_gateway::ID,
        }
        .to_account_metas(None),
        data: solana_axelar_gateway::instruction::TransferOperatorship {}.data(),
    };

    harness.ctx.process_and_validate_instruction_chain(&[(
        &ix,
        &[Check::err(gateway_err(
            GatewayError::InvalidOperatorOrAuthorityAccount,
        ))],
    )]);
}

#[test]
fn transfer_operatorship_same_operator() {
    let harness = GatewayTestHarness::new();

    let config: GatewayConfig = harness
        .get_account_as(&harness.gateway.root)
        .expect("gateway config should exist");

    // Try to transfer to the same operator
    let program_data = anchor_lang::prelude::bpf_loader_upgradeable::get_program_data_address(
        &solana_axelar_gateway::ID,
    );

    let (event_authority, _, _) =
        mollusk_test_utils::get_event_authority_and_program_accounts(&solana_axelar_gateway::ID);

    let ix = solana_sdk::instruction::Instruction {
        program_id: solana_axelar_gateway::ID,
        accounts: solana_axelar_gateway::accounts::TransferOperatorship {
            gateway_root_pda: harness.gateway.root,
            operator_or_upgrade_authority: harness.operator,
            program_data,
            new_operator: config.operator,
            event_authority,
            program: solana_axelar_gateway::ID,
        }
        .to_account_metas(None),
        data: solana_axelar_gateway::instruction::TransferOperatorship {}.data(),
    };

    // The constraint `new_operator.key() != gateway_root_pda.load()?.operator.key()`
    // returns ProgramError::InvalidInstructionData on failure.
    harness.ctx.process_and_validate_instruction_chain(&[(
        &ix,
        &[Check::err(
            solana_sdk::program_error::ProgramError::InvalidInstructionData,
        )],
    )]);
}
