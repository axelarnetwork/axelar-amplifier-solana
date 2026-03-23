#![cfg(test)]
#![allow(clippy::indexing_slicing)]
//! TEMPORARY: Test file using the new test harness. The tests will be split up into multiple files as the harness gets adopted.

use anchor_lang::prelude::AccountMeta;
use anchor_spl::token_2022;
use mollusk_harness::{ItsTestHarness, TestHarness};
use mollusk_svm::result::Check;
use solana_axelar_gateway::executable::{ExecutablePayload, ExecutablePayloadEncodingScheme};
use solana_program::program_pack::IsInitialized;
use solana_sdk::pubkey::Pubkey;

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
#[ignore = "TODO: should work, check if this is an Anchor bug"]
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

    // Assert transfer — the ATA authority is now a PDA derived from the
    // destination program, not the program ID itself
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

/// Test that the destination program can spend received ITS tokens.
/// The ATA authority is a PDA derived from the destination program, so the
/// program can sign for it via `invoke_signed` and transfer the tokens.
#[test]
fn execute_interchain_transfer_with_data_destination_program_can_spend_tokens() {
    let mut its_harness = ItsTestHarness::new();

    // Init memo
    its_harness.ensure_memo_program_initialized();
    let counter_pda = solana_axelar_memo::Counter::find_pda().0;

    // Deploy token + set trusted chain
    let token_id = its_harness.ensure_test_interchain_token();
    let source_chain = "ethereum";
    let source_address = "ethereum_address_123";
    let receiver = solana_axelar_memo::ID;
    let transfer_amount = 1_000_000u64;

    its_harness.ensure_trusted_chain(source_chain);

    let token_mint =
        solana_axelar_its::TokenManager::find_token_mint(token_id, its_harness.its_root).0;

    // Create a target wallet and its ATA to receive tokens from the memo program
    let target_wallet = its_harness.get_new_wallet();
    let (target_ata, _) =
        its_harness.get_or_create_ata_2022_account(its_harness.payer, target_wallet, token_mint);

    // Build data payload:
    // - memo string = "transfer" (triggers the memo handler to transfer tokens)
    // - accounts = [counter_pda, target_ata]
    //   The memo handler will transfer received tokens from its ATA to target_ata,
    //   signing with the token authority PDA.
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

    // Execute transfer - the memo program will receive tokens and transfer them
    // to target_ata, signing as the token authority PDA.
    its_harness.execute_gmp_transfer(
        token_id,
        source_chain,
        source_address,
        receiver,
        transfer_amount,
        Some((data, memo_accounts)),
    );

    // Verify the memo program successfully transferred tokens to the target
    let target_ata_data = its_harness.get_ata_2022_data(target_wallet, token_mint);
    assert_eq!(
        target_ata_data.amount, transfer_amount,
        "target wallet should have received the tokens from the memo program"
    );

    // Verify the memo program's ATA is now empty (tokens were moved, not duplicated)
    let destination_token_authority =
        solana_axelar_its::instructions::destination_token_authority_pda(&receiver);
    let source_ata_data = its_harness.get_ata_2022_data(destination_token_authority, token_mint);
    assert_eq!(
        source_ata_data.amount, 0,
        "memo program's ATA should be empty after transferring tokens out"
    );
}

/// Test that multiple CPI interchain transfers to the same destination program
/// reuse the same ATA (via init_if_needed) and the program can spend tokens
/// from each transfer.
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

    // Create a target wallet to receive tokens
    let target_wallet = its_harness.get_new_wallet();
    let (target_ata, _) =
        its_harness.get_or_create_ata_2022_account(its_harness.payer, target_wallet, token_mint);

    // First transfer: 1_000_000 tokens
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

    // Verify first transfer landed
    let target_ata_data = its_harness.get_ata_2022_data(target_wallet, token_mint);
    assert_eq!(target_ata_data.amount, first_amount);

    // Second transfer: 2_000_000 tokens (reuses the same ATA)
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

    // Verify cumulative amount on target
    let target_ata_data = its_harness.get_ata_2022_data(target_wallet, token_mint);
    assert_eq!(
        target_ata_data.amount,
        first_amount + second_amount,
        "target should have received tokens from both transfers"
    );

    // Verify memo program's ATA is empty after both transfers
    let destination_token_authority =
        solana_axelar_its::instructions::destination_token_authority_pda(&receiver);
    let source_ata_data = its_harness.get_ata_2022_data(destination_token_authority, token_mint);
    assert_eq!(
        source_ata_data.amount, 0,
        "memo program's ATA should be empty after transferring all tokens out"
    );

    // Verify counter was incremented twice
    let counter_account: solana_axelar_memo::Counter = its_harness
        .get_account_as(&counter_pda)
        .expect("counter account should exist");
    assert_eq!(counter_account.counter, 2, "counter should be 2");
}

/// Test that using the program ID as destination_token_authority (the old
/// broken behavior) is rejected for CPI transfers.
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
    let memo_string = "🫆🫆🫆".as_bytes().to_vec();
    let memo_accounts = vec![AccountMeta::new(counter_pda, false)];
    let data = ExecutablePayload::new(
        &memo_string,
        &memo_accounts,
        ExecutablePayloadEncodingScheme::Borsh,
    )
    .encode()
    .expect("failed to encode executable payload");

    // Use the program ID directly as authority (the old broken behavior).
    // Should be rejected with InvalidDestinationTokenAuthority.
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

    assert!(
        result.program_result.is_err(),
        "should reject program ID as destination_token_authority for CPI transfers"
    );
}

/// Test that using a wrong PDA (derived from wrong seeds) as
/// destination_token_authority is rejected for CPI transfers.
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
    let memo_string = "🫆🫆🫆".as_bytes().to_vec();
    let memo_accounts = vec![AccountMeta::new(counter_pda, false)];
    let data = ExecutablePayload::new(
        &memo_string,
        &memo_accounts,
        ExecutablePayloadEncodingScheme::Borsh,
    )
    .encode()
    .expect("failed to encode executable payload");

    // Use a PDA derived from wrong seeds.
    // Should be rejected with InvalidDestinationTokenAuthority.
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

    assert!(
        result.program_result.is_err(),
        "should reject wrong PDA as destination_token_authority for CPI transfers"
    );
}

/// Test that a malicious relayer cannot redirect tokens in a simple (no-data)
/// transfer by passing an arbitrary destination_token_authority.
#[test]
fn reject_simple_transfer_when_authority_differs_from_destination() {
    let mut its_harness = ItsTestHarness::new();

    let token_id = its_harness.ensure_test_interchain_token();
    let source_chain = "ethereum";
    its_harness.ensure_trusted_chain(source_chain);

    let receiver = its_harness.get_new_wallet();
    let transfer_amount = 1_000_000u64;

    // Pass an attacker-controlled authority instead of the destination wallet
    let attacker = its_harness.get_new_wallet();

    let result = its_harness.execute_gmp_transfer_with_authority(
        token_id,
        source_chain,
        "ethereum_address_123",
        receiver,
        transfer_amount,
        None, // no data = simple transfer
        attacker,
        &[Check::err(
            anchor_lang::error::Error::from(
                solana_axelar_its::ItsError::InvalidDestinationTokenAuthority,
            )
            .into(),
        )],
    );

    assert!(
        result.program_result.is_err(),
        "should reject simple transfer when destination_token_authority != destination"
    );
}

/// Test that a simple (no-data) transfer to a program succeeds by using the
/// program's token authority PDA as the ATA authority, so the program can later
/// spend the tokens via `invoke_signed`.
#[test]
fn execute_simple_interchain_transfer_to_program() {
    let mut its_harness = ItsTestHarness::new();

    let token_id = its_harness.ensure_test_interchain_token();
    let source_chain = "ethereum";
    its_harness.ensure_trusted_chain(source_chain);

    its_harness.ensure_memo_program_initialized();
    let receiver = solana_axelar_memo::ID;
    let transfer_amount = 1_000_000u64;

    // The harness will derive the PDA authority automatically since receiver is a program
    let result = its_harness.execute_gmp_transfer(
        token_id,
        source_chain,
        "ethereum_address_123",
        receiver,
        transfer_amount,
        None,
    );

    assert!(
        result.program_result.is_ok(),
        "simple transfer to a program should succeed using the token authority PDA"
    );

    // Verify tokens landed in the ATA owned by the program's token authority PDA
    let token_authority =
        solana_axelar_its::instructions::destination_token_authority_pda(&receiver);
    let token_mint = its_harness.token_mint_for_id(token_id);
    let ata_data = its_harness.get_ata_2022_data(token_authority, token_mint);
    assert_eq!(ata_data.amount, transfer_amount);
    assert_eq!(ata_data.owner, token_authority);
}

/// Test CPI interchain transfer with a canonical (LockUnlock) token.
/// Unlike MintBurn tokens, LockUnlock tokens are transferred from the token
/// manager's ATA (unlocked) rather than minted fresh.
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

    // Execute: tokens should be unlocked from token_manager_ata → memo's ATA → target
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
    assert_eq!(
        target_ata_data.amount, transfer_amount,
        "target wallet should have received unlocked tokens"
    );

    // Verify memo program's ATA is empty
    let destination_token_authority =
        solana_axelar_its::instructions::destination_token_authority_pda(&receiver);
    let source_ata_data = its_harness.get_ata_2022_data(destination_token_authority, token_mint);
    assert_eq!(
        source_ata_data.amount, 0,
        "memo program's ATA should be empty after transferring tokens out"
    );

    // Verify token manager still has the remaining locked tokens
    let tm_ata_data: anchor_spl::token_interface::TokenAccount = its_harness
        .get_account_as(&token_manager_ata)
        .expect("token manager ATA should exist");
    assert_eq!(
        tm_ata_data.amount,
        lock_amount - transfer_amount,
        "token manager should have remaining locked tokens"
    );
}
