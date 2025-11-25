#![cfg(test)]
#![allow(clippy::indexing_slicing)]

use anchor_lang::prelude::AccountMeta;
use mollusk_harness::{ItsTestHarness, TestHarness};
use solana_axelar_gateway::executable::{ExecutablePayload, ExecutablePayloadEncodingScheme};
use solana_program::program_pack::IsInitialized;

#[test]
fn test_init_gives_user_role_to_operator() {
    let its_harness = ItsTestHarness::new();

    let user_roles_pda =
        solana_axelar_its::UserRoles::find_pda(&its_harness.its_root, &its_harness.operator).0;
    let user_roles: solana_axelar_its::UserRoles = its_harness
        .get_account_as(&user_roles_pda)
        .expect("user roles account should exist");

    assert_eq!(
        user_roles.roles,
        solana_axelar_its::Roles::OPERATOR,
        "user should be an operator"
    );
}

#[test]
fn test_local_deploy_interchain_token() {
    let its_harness = ItsTestHarness::new();

    its_harness.ensure_test_interchain_token();
}

#[test]
fn test_execute_interchain_transfer() {
    let mut its_harness = ItsTestHarness::new();

    let token_id = its_harness.ensure_test_interchain_token();
    let source_chain = "ethereum";
    let source_address = "ethereum_address_123";
    let receiver = its_harness.get_new_wallet();
    let transfer_amount = 1_000_000u64;
    let data = None;

    its_harness.ensure_trusted_chain(source_chain);

    its_harness.execute_gmp_transfer(
        token_id,
        source_chain,
        source_address,
        receiver,
        transfer_amount,
        data,
    );

    let token_mint =
        solana_axelar_its::TokenManager::find_token_mint(token_id, its_harness.its_root).0;
    let destination_ata_data = its_harness.get_ata_2022_data(receiver, token_mint);

    assert_eq!(destination_ata_data.amount, transfer_amount);
    assert_eq!(destination_ata_data.mint, token_mint);
    assert_eq!(destination_ata_data.owner, receiver);
    assert!(destination_ata_data.is_initialized());
}

#[test]
fn test_execute_interchain_transfer_with_data() {
    let mut its_harness = ItsTestHarness::new();

    // Init memo
    its_harness.ensure_memo_program_initialized();
    let counter_pda = solana_axelar_memo::Counter::get_pda().0;

    // Transfer

    let token_id = its_harness.ensure_test_interchain_token();
    let source_chain = "ethereum";
    let source_address = "ethereum_address_123";
    let receiver = solana_axelar_memo::ID;
    let transfer_amount = 1_000_000u64;

    // Data

    // String to print
    #[allow(clippy::non_ascii_literal)]
    let memo_string = "🫆🫆🫆".as_bytes().to_vec();
    // Custom accounts
    let memo_accounts = vec![AccountMeta::new(counter_pda, false)];
    // Payload encoding
    let data = ExecutablePayload::new(
        &memo_string,
        &memo_accounts,
        ExecutablePayloadEncodingScheme::Borsh,
    )
    .encode()
    .expect("failed to encode executable payload");

    its_harness.ensure_trusted_chain(source_chain);

    // Execute transfer

    its_harness.execute_gmp_transfer(
        token_id,
        source_chain,
        source_address,
        receiver,
        transfer_amount,
        Some((data, memo_accounts)),
    );

    // Assert transfer

    let token_mint =
        solana_axelar_its::TokenManager::find_token_mint(token_id, its_harness.its_root).0;
    let destination_ata_data = its_harness.get_ata_2022_data(receiver, token_mint);

    assert_eq!(destination_ata_data.amount, transfer_amount);
    assert_eq!(destination_ata_data.mint, token_mint);
    assert_eq!(destination_ata_data.owner, receiver);
    assert!(destination_ata_data.is_initialized());

    // Assert memo execution

    let counter_account: solana_axelar_memo::Counter = its_harness
        .get_account_as(&counter_pda)
        .expect("counter account should exist");
    assert_eq!(
        counter_account.counter, 1,
        "counter should have been incremented"
    );
}
