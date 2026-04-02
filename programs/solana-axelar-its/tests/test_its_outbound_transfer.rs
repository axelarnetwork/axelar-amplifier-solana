#![cfg(test)]
#![allow(clippy::indexing_slicing)]

use anchor_spl::token_2022;
use mollusk_harness::{ItsTestHarness, TestHarness};
use mollusk_svm::result::Check;

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
    let sender = solana_axelar_memo::Counter::find_pda().0;
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
    let sender = solana_axelar_memo::Counter::find_pda().0;
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

#[test]
fn reject_outbound_interchain_transfer_empty_destination() {
    let mut its_harness = ItsTestHarness::new();

    let token_id = its_harness.ensure_test_interchain_token();
    let token_mint = its_harness.token_mint_for_id(token_id);

    let sender = its_harness.get_new_wallet();
    let sender_ata = its_harness
        .get_or_create_ata_2022_account(its_harness.payer, sender, token_mint)
        .0;
    its_harness.ensure_mint_test_interchain_token(token_id, 500_000, sender_ata);

    its_harness.ensure_trusted_chain("ethereum");

    let (ix, _) = solana_axelar_its::instructions::make_interchain_transfer_instruction(
        token_id,
        100_000,
        token_2022::ID,
        its_harness.payer,
        sender,
        "ethereum".to_owned(),
        vec![], // empty destination address
        0,
        None,
        None,
        None,
    );

    its_harness.ctx.process_and_validate_instruction(
        &ix,
        &[Check::err(
            solana_axelar_its::ItsError::InvalidDestinationAddress.into(),
        )],
    );
}
