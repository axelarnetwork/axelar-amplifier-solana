#![cfg(test)]
#![allow(clippy::indexing_slicing)]

use anchor_lang::prelude::AccountMeta;
use mollusk_harness::{ItsTestHarness, TestHarness};
use mollusk_svm::result::Check;
use solana_axelar_gateway::executable::{ExecutablePayload, ExecutablePayloadEncodingScheme};
use solana_program::program_pack::IsInitialized;
use solana_sdk::pubkey::Pubkey;

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
fn test_execute_interchain_transfer_existing_ata() {
    let mut its_harness = ItsTestHarness::new();

    let token_id = its_harness.ensure_test_interchain_token();
    let source_chain = "ethereum";
    let source_address = "ethereum_address_123";
    let receiver = its_harness.get_new_wallet();
    let transfer_amount = 1_000_000u64;
    let data = None;
    let token_mint =
        solana_axelar_its::TokenManager::find_token_mint(token_id, its_harness.its_root).0;

    // Create ATA
    let (destination_ata, _) =
        its_harness.get_or_create_ata_2022_account(its_harness.payer, receiver, token_mint);

    // Pre-mint some tokens to the destination ATA
    let initial_amount = 500_000u64;
    its_harness.ensure_mint_test_interchain_token(token_id, initial_amount, destination_ata);

    let destination_ata_data = its_harness.get_ata_2022_data(receiver, token_mint);
    assert_eq!(destination_ata_data.amount, initial_amount);
    assert!(destination_ata_data.is_initialized());

    // Execute transfer
    its_harness.ensure_trusted_chain(source_chain);
    its_harness.execute_gmp_transfer(
        token_id,
        source_chain,
        source_address,
        receiver,
        transfer_amount,
        data,
    );

    // Check final balance
    let destination_ata_data = its_harness.get_ata_2022_data(receiver, token_mint);
    assert_eq!(
        destination_ata_data.amount,
        initial_amount + transfer_amount
    );
    assert_eq!(destination_ata_data.mint, token_mint);
    assert_eq!(destination_ata_data.owner, receiver);
    assert!(destination_ata_data.is_initialized());
}

#[test]
fn execute_interchain_transfer_payer_equals_destination() {
    let mut its_harness = ItsTestHarness::new();

    let token_id = its_harness.ensure_test_interchain_token();
    let source_chain = "ethereum";
    let source_address = "ethereum_address_123";
    // Receiver is the payer of the transaction
    // Useful if the user wants to relay the message themselves
    let receiver = its_harness.payer;
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
fn execute_interchain_transfer_with_data() {
    let mut its_harness = ItsTestHarness::new();

    // Init memo
    its_harness.ensure_memo_program_initialized();
    let counter_pda = solana_axelar_memo::Counter::find_pda().0;

    // Transfer
    let token_id = its_harness.ensure_test_interchain_token();
    let source_chain = "ethereum";
    let source_address = "ethereum_address_123";
    let receiver = solana_axelar_memo::ID;
    let transfer_amount = 1_000_000u64;

    // Data
    #[allow(clippy::non_ascii_literal)]
    let memo_string = "\u{1fac6}\u{1fac6}\u{1fac6}".as_bytes().to_vec();
    let memo_accounts = vec![AccountMeta::new(counter_pda, false)];
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
    let destination_token_authority =
        solana_axelar_its::instructions::destination_token_authority_pda(&receiver);
    let destination_ata_data =
        its_harness.get_ata_2022_data(destination_token_authority, token_mint);

    assert_eq!(destination_ata_data.amount, transfer_amount);
    assert_eq!(destination_ata_data.mint, token_mint);
    assert_eq!(destination_ata_data.owner, destination_token_authority);
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

#[test]
fn execute_interchain_transfer_with_data_destination_program_can_spend_tokens() {
    let mut its_harness = ItsTestHarness::new();

    its_harness.ensure_memo_program_initialized();
    let counter_pda = solana_axelar_memo::Counter::find_pda().0;

    let token_id = its_harness.ensure_test_interchain_token();
    let source_chain = "ethereum";
    let source_address = "ethereum_address_123";
    let receiver = solana_axelar_memo::ID;
    let transfer_amount = 1_000_000u64;

    its_harness.ensure_trusted_chain(source_chain);

    let token_mint =
        solana_axelar_its::TokenManager::find_token_mint(token_id, its_harness.its_root).0;

    let target_wallet = its_harness.get_new_wallet();
    let (target_ata, _) =
        its_harness.get_or_create_ata_2022_account(its_harness.payer, target_wallet, token_mint);

    let memo_string = b"transfer".to_vec();
    let memo_accounts = vec![
        AccountMeta::new(counter_pda, false),
        AccountMeta::new(target_ata, false),
    ];
    let data = ExecutablePayload::new(
        &memo_string,
        &memo_accounts,
        ExecutablePayloadEncodingScheme::Borsh,
    )
    .encode()
    .expect("failed to encode executable payload");

    its_harness.execute_gmp_transfer(
        token_id,
        source_chain,
        source_address,
        receiver,
        transfer_amount,
        Some((data, memo_accounts)),
    );

    let target_ata_data = its_harness.get_ata_2022_data(target_wallet, token_mint);
    assert_eq!(target_ata_data.amount, transfer_amount);

    let destination_token_authority =
        solana_axelar_its::instructions::destination_token_authority_pda(&receiver);
    let source_ata_data = its_harness.get_ata_2022_data(destination_token_authority, token_mint);
    assert_eq!(source_ata_data.amount, 0);
}

#[test]
fn execute_multiple_interchain_transfers_with_data_to_same_program() {
    let mut its_harness = ItsTestHarness::new();

    its_harness.ensure_memo_program_initialized();
    let counter_pda = solana_axelar_memo::Counter::find_pda().0;

    let token_id = its_harness.ensure_test_interchain_token();
    let source_chain = "ethereum";
    let receiver = solana_axelar_memo::ID;

    its_harness.ensure_trusted_chain(source_chain);

    let token_mint =
        solana_axelar_its::TokenManager::find_token_mint(token_id, its_harness.its_root).0;

    let target_wallet = its_harness.get_new_wallet();
    let (target_ata, _) =
        its_harness.get_or_create_ata_2022_account(its_harness.payer, target_wallet, token_mint);

    // First transfer
    let first_amount = 1_000_000u64;
    let memo_accounts = vec![
        AccountMeta::new(counter_pda, false),
        AccountMeta::new(target_ata, false),
    ];
    let data = ExecutablePayload::new(
        b"transfer".as_ref(),
        &memo_accounts,
        ExecutablePayloadEncodingScheme::Borsh,
    )
    .encode()
    .expect("failed to encode executable payload");

    its_harness.execute_gmp_transfer(
        token_id,
        source_chain,
        "ethereum_address_first",
        receiver,
        first_amount,
        Some((data, memo_accounts)),
    );

    let target_ata_data = its_harness.get_ata_2022_data(target_wallet, token_mint);
    assert_eq!(target_ata_data.amount, first_amount);

    // Second transfer
    let second_amount = 2_000_000u64;
    let memo_accounts = vec![
        AccountMeta::new(counter_pda, false),
        AccountMeta::new(target_ata, false),
    ];
    let data = ExecutablePayload::new(
        b"transfer".as_ref(),
        &memo_accounts,
        ExecutablePayloadEncodingScheme::Borsh,
    )
    .encode()
    .expect("failed to encode executable payload");

    its_harness.execute_gmp_transfer(
        token_id,
        source_chain,
        "ethereum_address_second",
        receiver,
        second_amount,
        Some((data, memo_accounts)),
    );

    let target_ata_data = its_harness.get_ata_2022_data(target_wallet, token_mint);
    assert_eq!(target_ata_data.amount, first_amount + second_amount);

    let destination_token_authority =
        solana_axelar_its::instructions::destination_token_authority_pda(&receiver);
    let source_ata_data = its_harness.get_ata_2022_data(destination_token_authority, token_mint);
    assert_eq!(source_ata_data.amount, 0);

    let counter_account: solana_axelar_memo::Counter = its_harness
        .get_account_as(&counter_pda)
        .expect("counter account should exist");
    assert_eq!(counter_account.counter, 2);
}

#[test]
fn reject_interchain_transfer_with_data_when_authority_is_program_id() {
    let mut its_harness = ItsTestHarness::new();

    its_harness.ensure_memo_program_initialized();
    let counter_pda = solana_axelar_memo::Counter::find_pda().0;

    let token_id = its_harness.ensure_test_interchain_token();
    let source_chain = "ethereum";
    its_harness.ensure_trusted_chain(source_chain);

    let receiver = solana_axelar_memo::ID;
    let transfer_amount = 1_000_000u64;

    #[allow(clippy::non_ascii_literal)]
    let memo_string = "\u{1fac6}\u{1fac6}\u{1fac6}".as_bytes().to_vec();
    let memo_accounts = vec![AccountMeta::new(counter_pda, false)];
    let data = ExecutablePayload::new(
        &memo_string,
        &memo_accounts,
        ExecutablePayloadEncodingScheme::Borsh,
    )
    .encode()
    .expect("failed to encode executable payload");

    let result = its_harness.execute_gmp_transfer_with_authority(
        token_id,
        source_chain,
        "ethereum_address_123",
        receiver,
        transfer_amount,
        Some((data, memo_accounts)),
        receiver, // program ID as authority
        &[Check::err(
            anchor_lang::error::Error::from(
                solana_axelar_its::ItsError::InvalidDestinationTokenAuthority,
            )
            .into(),
        )],
    );

    assert!(result.program_result.is_err());
}

#[test]
fn reject_interchain_transfer_with_data_when_authority_is_wrong_pda() {
    let mut its_harness = ItsTestHarness::new();

    its_harness.ensure_memo_program_initialized();
    let counter_pda = solana_axelar_memo::Counter::find_pda().0;

    let token_id = its_harness.ensure_test_interchain_token();
    let source_chain = "ethereum";
    its_harness.ensure_trusted_chain(source_chain);

    let receiver = solana_axelar_memo::ID;
    let transfer_amount = 1_000_000u64;

    #[allow(clippy::non_ascii_literal)]
    let memo_string = "\u{1fac6}\u{1fac6}\u{1fac6}".as_bytes().to_vec();
    let memo_accounts = vec![AccountMeta::new(counter_pda, false)];
    let data = ExecutablePayload::new(
        &memo_string,
        &memo_accounts,
        ExecutablePayloadEncodingScheme::Borsh,
    )
    .encode()
    .expect("failed to encode executable payload");

    let wrong_authority = Pubkey::find_program_address(&[b"wrong-seed"], &receiver).0;

    let result = its_harness.execute_gmp_transfer_with_authority(
        token_id,
        source_chain,
        "ethereum_address_123",
        receiver,
        transfer_amount,
        Some((data, memo_accounts)),
        wrong_authority,
        &[Check::err(
            anchor_lang::error::Error::from(
                solana_axelar_its::ItsError::InvalidDestinationTokenAuthority,
            )
            .into(),
        )],
    );

    assert!(result.program_result.is_err());
}

#[test]
fn reject_simple_transfer_when_authority_differs_from_destination() {
    let mut its_harness = ItsTestHarness::new();

    let token_id = its_harness.ensure_test_interchain_token();
    let source_chain = "ethereum";
    its_harness.ensure_trusted_chain(source_chain);

    let receiver = its_harness.get_new_wallet();
    let transfer_amount = 1_000_000u64;
    let attacker = its_harness.get_new_wallet();

    let result = its_harness.execute_gmp_transfer_with_authority(
        token_id,
        source_chain,
        "ethereum_address_123",
        receiver,
        transfer_amount,
        None,
        attacker,
        &[Check::err(
            anchor_lang::error::Error::from(
                solana_axelar_its::ItsError::InvalidDestinationTokenAuthority,
            )
            .into(),
        )],
    );

    assert!(result.program_result.is_err());
}

#[test]
fn execute_simple_interchain_transfer_to_program() {
    let mut its_harness = ItsTestHarness::new();

    let token_id = its_harness.ensure_test_interchain_token();
    let source_chain = "ethereum";
    its_harness.ensure_trusted_chain(source_chain);

    its_harness.ensure_memo_program_initialized();
    let receiver = solana_axelar_memo::ID;
    let transfer_amount = 1_000_000u64;

    let result = its_harness.execute_gmp_transfer(
        token_id,
        source_chain,
        "ethereum_address_123",
        receiver,
        transfer_amount,
        None,
    );

    assert!(result.program_result.is_ok());

    let token_authority =
        solana_axelar_its::instructions::destination_token_authority_pda(&receiver);
    let token_mint = its_harness.token_mint_for_id(token_id);
    let ata_data = its_harness.get_ata_2022_data(token_authority, token_mint);
    assert_eq!(ata_data.amount, transfer_amount);
    assert_eq!(ata_data.owner, token_authority);
}

#[test]
fn execute_interchain_transfer_with_data_lock_unlock_token() {
    use anchor_spl::associated_token::get_associated_token_address_with_program_id;
    use anchor_spl::token_2022::spl_token_2022;

    let mut its_harness = ItsTestHarness::new();

    its_harness.ensure_memo_program_initialized();
    let counter_pda = solana_axelar_memo::Counter::find_pda().0;

    let source_chain = "ethereum";
    its_harness.ensure_trusted_chain(source_chain);

    // Create a canonical SPL token (LockUnlock type)
    let mint_authority = its_harness.get_new_wallet();
    let (token_mint, token_id) = its_harness.ensure_test_registered_canonical_token(mint_authority);

    // Fund the token manager's ATA with tokens (simulating previously locked tokens)
    let token_manager_pda =
        solana_axelar_its::TokenManager::find_pda(token_id, its_harness.its_root).0;
    let token_manager_ata = get_associated_token_address_with_program_id(
        &token_manager_pda,
        &token_mint,
        &spl_token_2022::ID,
    );

    let lock_amount = 5_000_000u64;
    let mint_ix = spl_token_2022::instruction::mint_to(
        &spl_token_2022::ID,
        &token_mint,
        &token_manager_ata,
        &mint_authority,
        &[],
        lock_amount,
    )
    .unwrap();

    its_harness
        .ctx
        .process_and_validate_instruction(&mint_ix, &[Check::success()]);

    // Create target wallet to receive tokens
    let receiver = solana_axelar_memo::ID;
    let target_wallet = its_harness.get_new_wallet();
    let (target_ata, _) =
        its_harness.get_or_create_ata_2022_account(its_harness.payer, target_wallet, token_mint);

    // Build CPI transfer payload
    let transfer_amount = 1_000_000u64;
    let memo_accounts = vec![
        AccountMeta::new(counter_pda, false),
        AccountMeta::new(target_ata, false),
    ];
    let data = ExecutablePayload::new(
        b"transfer".as_ref(),
        &memo_accounts,
        ExecutablePayloadEncodingScheme::Borsh,
    )
    .encode()
    .expect("failed to encode executable payload");

    its_harness.execute_gmp_transfer(
        token_id,
        source_chain,
        "ethereum_address_123",
        receiver,
        transfer_amount,
        Some((data, memo_accounts)),
    );

    // Verify target received tokens
    let target_ata_data = its_harness.get_ata_2022_data(target_wallet, token_mint);
    assert_eq!(target_ata_data.amount, transfer_amount);

    // Verify memo program's ATA is empty
    let destination_token_authority =
        solana_axelar_its::instructions::destination_token_authority_pda(&receiver);
    let source_ata_data = its_harness.get_ata_2022_data(destination_token_authority, token_mint);
    assert_eq!(source_ata_data.amount, 0);

    // Verify token manager still has the remaining locked tokens
    let tm_ata_data: anchor_spl::token_interface::TokenAccount = its_harness
        .get_account_as(&token_manager_ata)
        .expect("token manager ATA should exist");
    assert_eq!(tm_ata_data.amount, lock_amount - transfer_amount);
}

#[test]
fn reject_execute_interchain_transfer_with_zero_amount() {
    let mut its_harness = ItsTestHarness::new();

    let token_id = its_harness.ensure_test_interchain_token();
    its_harness.ensure_trusted_chain("ethereum");

    let receiver = its_harness.get_new_wallet();

    let result = its_harness.execute_gmp_transfer_with_authority(
        token_id,
        "ethereum",
        "eth_addr_123",
        receiver,
        0, // zero amount
        None,
        receiver,
        &[Check::err(
            solana_axelar_its::ItsError::InvalidAmount.into(),
        )],
    );

    assert!(result.program_result.is_err());
}
