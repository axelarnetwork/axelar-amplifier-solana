#![cfg(test)]
#![allow(clippy::indexing_slicing)]
//! TEMPORARY: Test file using the new test harness. The tests will be split up into multiple files as the harness gets adopted.

use anchor_lang::{prelude::AccountMeta, AnchorSerialize};
use anchor_spl::token_2022;
use mollusk_harness::{ItsTestHarness, TestHarness};
use mollusk_svm::result::Check;
use solana_axelar_gateway::executable::{ExecutablePayload, ExecutablePayloadEncodingScheme};
use solana_program::program_pack::IsInitialized;

#[test]
fn test_local_deploy_interchain_token() {
    let its_harness = ItsTestHarness::new();

    let _token_id = its_harness.ensure_test_interchain_token();
}

#[test]
fn test_mint_interchain_tokens() {
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

// Outbound transfers

#[test]
fn test_user_interchain_transfer() {
    let mut its_harness = ItsTestHarness::new();

    // Create token
    let token_id = its_harness.ensure_test_interchain_token();
    let token_mint = its_harness.token_mint_for_id(token_id);

    // Mint tokens to sender
    let mint_amount = 500_000u64;
    let sender = its_harness.get_new_wallet();
    let sender_ata = its_harness
        .get_or_create_ata_2022_account(its_harness.payer, sender, token_mint)
        .0;

    its_harness.ensure_mint_test_interchain_token(token_id, mint_amount, sender_ata);

    // Transfer
    let transfer_amount = 300_000u64;
    let destination_chain = "ethereum";
    let destination_address = b"ethereum_address_456".to_vec();
    let gas_value = 10_000u64;

    its_harness.ensure_trusted_chain(destination_chain);

    its_harness.ensure_outgoing_user_interchain_transfer(
        token_id,
        transfer_amount,
        token_2022::ID,
        its_harness.payer,
        sender,
        destination_chain.to_owned(),
        destination_address,
        gas_value,
    );
}

#[test]
fn test_cpi_interchain_transfer() {
    let mut its_harness = ItsTestHarness::new();
    its_harness.ensure_memo_program_initialized();

    // Create token
    let token_id = its_harness.ensure_test_interchain_token();
    let token_mint = its_harness.token_mint_for_id(token_id);

    // Mint tokens to the CPI caller PDA
    let mint_amount = 500_000u64;
    let sender = solana_axelar_memo::Counter::get_pda().0;
    let sender_ata = its_harness
        .get_or_create_ata_2022_account(its_harness.payer, sender, token_mint)
        .0;

    its_harness.ensure_mint_test_interchain_token(token_id, mint_amount, sender_ata);

    // Transfer
    let transfer_amount = 300_000u64;
    let destination_chain = "ethereum";
    let destination_address = b"ethereum_address_456".to_vec();
    let gas_value = 10_000u64;
    // CPI info
    let caller_program_id = solana_axelar_memo::ID;
    let caller_pda_seeds = solana_axelar_memo::Counter::pda_seeds()
        .iter()
        .map(|s| s.to_vec())
        .collect::<Vec<Vec<u8>>>();

    its_harness.ensure_trusted_chain(destination_chain);

    its_harness.ensure_outgoing_interchain_transfer(
        token_id,
        transfer_amount,
        token_2022::ID,
        its_harness.payer,
        sender,
        destination_chain.to_owned(),
        destination_address,
        gas_value,
        Some(caller_program_id),
        Some(caller_pda_seeds),
        None,
    );
}

#[test]
fn test_cpi_interchain_transfer_invalid_pda_arguments() {
    let mut its_harness = ItsTestHarness::new();
    its_harness.ensure_memo_program_initialized();

    // Create token
    let token_id = its_harness.ensure_test_interchain_token();
    let token_mint = its_harness.token_mint_for_id(token_id);

    // Mint tokens to the CPI caller PDA
    let mint_amount = 500_000u64;
    let sender = solana_axelar_memo::Counter::get_pda().0;
    let sender_ata = its_harness
        .get_or_create_ata_2022_account(its_harness.payer, sender, token_mint)
        .0;

    its_harness.ensure_mint_test_interchain_token(token_id, mint_amount, sender_ata);

    // Transfer
    let transfer_amount = 300_000u64;
    let destination_chain = "ethereum";
    let destination_address = b"ethereum_address_456".to_vec();
    let gas_value = 10_000u64;
    // CPI info
    let caller_program_id = solana_axelar_memo::ID;
    let caller_pda_seeds = vec![b"invalid_seed".to_vec()]; // invalid seeds!

    its_harness.ensure_trusted_chain(destination_chain);

    let (ix, _) = solana_axelar_its::instructions::make_interchain_transfer_instruction(
        token_id,
        transfer_amount,
        token_2022::ID,
        its_harness.payer,
        sender,
        destination_chain.to_owned(),
        destination_address,
        gas_value,
        Some(caller_program_id),
        Some(caller_pda_seeds),
        None,
    );

    its_harness.ctx.process_and_validate_instruction_chain(&[(
        &ix,
        &[Check::err(
            solana_axelar_its::ItsError::InvalidAccountData.into(),
        )],
    )]);
}

// Inbound transfers

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
    its_harness.ensure_test_discoverable_program_initialized();

    // Transfer

    let token_id = its_harness.ensure_test_interchain_token();
    let source_chain = "ethereum";
    let source_address = "ethereum_address_123";
    let receiver = solana_axelar_test_discoverable::ID;
    let transfer_amount = 1_000_000u64;

    // Data

    // String to print
    #[allow(clippy::non_ascii_literal)]
    let memo_string = String::from("🫆🫆🫆");
    let storage_id = 123;

    // Payload encoding
    let data = {
        let mut bytes = vec![];
        solana_axelar_test_discoverable::Payload {
            storage_id,
            memo: memo_string,
        }
        .serialize(&mut bytes)
        .expect("failed to encode executable payload");
        bytes
    };

    its_harness.ensure_trusted_chain(source_chain);

    // Execute transfer

    its_harness.execute_gmp_transfer(
        token_id,
        source_chain,
        source_address,
        receiver,
        transfer_amount,
        Some(data),
    );

    // Assert transfer

    let token_mint =
        solana_axelar_its::TokenManager::find_token_mint(token_id, its_harness.its_root).0;
    let destination_ata_data = its_harness.get_ata_2022_data(receiver, token_mint);
    let counter_pda = solana_axelar_test_discoverable::Counter::get_pda(storage_id).0;

    assert_eq!(destination_ata_data.amount, transfer_amount);
    assert_eq!(destination_ata_data.mint, token_mint);
    assert_eq!(destination_ata_data.owner, receiver);
    assert!(destination_ata_data.is_initialized());

    // Assert memo execution

    let counter_account: solana_axelar_test_discoverable::Counter = its_harness
        .get_account_as(&counter_pda)
        .expect("counter account should exist");
    assert_eq!(
        counter_account.counter, 1,
        "counter should have been incremented"
    );
}
