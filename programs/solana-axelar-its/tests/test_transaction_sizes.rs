#![cfg(test)]
//! Transaction size analysis for inbound GMP transactions.
//!
//! Solana legacy transactions are limited to **1232 bytes**.
//! This module computes the wire-format size of each inbound instruction
//! and reports the protocol limits for user payload + accounts.
//!
//! Covers:
//!   1. Gateway GMP Execute (generic destination program)
//!   2. ITS Execute → DeployInterchainToken
//!   3. ITS Execute → LinkToken
//!   4. ITS Execute → InterchainTransfer (without data)
//!   5. ITS Execute → InterchainTransfer (with data) — Level 1 & Level 2 ALT
//!
//! Run with:
//!   cargo test -p solana-axelar-its --test test_transaction_sizes -- --nocapture --test-threads=1

use std::collections::BTreeSet;

use anchor_lang::prelude::borsh;
use anchor_lang::{InstructionData, ToAccountMetas};
use anchor_spl::token_2022;
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::pubkey::Pubkey;

const SEPARATOR: &str = "========================================================================";

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const MAX_TX_SIZE: usize = 1232;

/// Typical chain name lengths
const CHAIN_ETHEREUM: &str = "ethereum";
const CHAIN_SOLANA: &str = "solana";

/// Typical ITS hub address (bech32, ~44 chars)
const TYPICAL_HUB_ADDRESS: &str = "axelar1abcdefghijklmnopqrstuvwxyz012345678abcd";

/// Typical cross-chain message ID (~66 chars, hex-encoded hash)
const TYPICAL_CC_ID: &str = "0xabcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789";

/// ITS global ALT: 7 readonly entries.
///   0. gateway_root_pda
///   1. gateway_event_authority
///   2. axelar_gateway_program
///   3. its_root_pda
///   4. associated_token_program
///   5. system_program
///   6. event_authority (ITS)
///   7. spl_token (Token program)
///   8. spl_token_2022 (Token2022 program)
///
/// Both token programs are included so the ALT works regardless of which
/// token standard a given token uses. Unused entries cost nothing.
///
/// NOT in ALT:
///   - program (ITS) — instruction's program_id, must be direct per v0 spec
///   - payer           — signer, must be direct
///
/// ALT overhead: 32 (address) + 1 (compact writable=0) + 1 (compact readonly len) + 9 = 43
/// Saved: 9 × 32 = 288 bytes. Net: 288 - 43 = 245 bytes.
const ITS_GLOBAL_ALT_ENTRIES: usize = 9;
const ITS_GLOBAL_ALT_OVERHEAD: usize = 32 + 1 + 0 + 1 + ITS_GLOBAL_ALT_ENTRIES; // 43
const ITS_GLOBAL_ALT_SAVINGS: usize = ITS_GLOBAL_ALT_ENTRIES * 32 - ITS_GLOBAL_ALT_OVERHEAD; // 245

// ---------------------------------------------------------------------------
// Size calculation helpers
// ---------------------------------------------------------------------------

fn compact_u16_len(val: usize) -> usize {
    if val < 0x80 {
        1
    } else if val < 0x4000 {
        2
    } else {
        3
    }
}

struct TxSizeBreakdown {
    total: usize,
    num_unique_accounts: usize,
    num_signers: usize,
    ix_data_len: usize,
}

/// Compute the legacy (no-ALT) transaction size for a single instruction.
fn compute_legacy_tx_size(ix: &Instruction) -> TxSizeBreakdown {
    let mut unique_keys = BTreeSet::new();
    let mut num_signers = 0usize;

    for meta in &ix.accounts {
        if unique_keys.insert(meta.pubkey) && meta.is_signer {
            num_signers += 1;
        }
    }
    unique_keys.insert(ix.program_id);

    let num_unique = unique_keys.len();
    let num_ix_accounts = ix.accounts.len();
    let data_len = ix.data.len();

    let total = compact_u16_len(num_signers) + num_signers * 64  // signatures
        + 3                                                       // header
        + compact_u16_len(num_unique) + num_unique * 32           // account keys
        + 32                                                      // blockhash
        + compact_u16_len(1)                                      // num instructions
        + 1                                                       // program_id_index
        + compact_u16_len(num_ix_accounts) + num_ix_accounts      // account indices
        + compact_u16_len(data_len) + data_len;                   // data

    TxSizeBreakdown {
        total,
        num_unique_accounts: num_unique,
        num_signers,
        ix_data_len: data_len,
    }
}

fn accounts_that_fit(remaining_bytes: usize) -> usize {
    remaining_bytes / 33 // 32 key + 1 index
}

/// Binary search for the maximum payload size that fits.
fn find_max_payload(fits: impl Fn(usize) -> bool) -> usize {
    let mut lo = 0usize;
    let mut hi = MAX_TX_SIZE;
    while lo < hi {
        let mid = (lo + hi + 1) / 2;
        if fits(mid) {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    lo
}

// ---------------------------------------------------------------------------
// Instruction builders
// ---------------------------------------------------------------------------

/// Builds a typical cross-chain Message for instruction parameters.
fn typical_cross_chain_message(payload_hash: [u8; 32], destination_program: &str) -> solana_axelar_std::Message {
    solana_axelar_std::Message {
        cc_id: solana_axelar_std::CrossChainId {
            chain: CHAIN_ETHEREUM.to_owned(),
            id: TYPICAL_CC_ID.to_owned(),
        },
        source_address: TYPICAL_HUB_ADDRESS.to_owned(),
        destination_chain: CHAIN_SOLANA.to_owned(),
        destination_address: destination_program.to_owned(),
        payload_hash,
    }
}

/// Builds an ITS Execute accounts struct with unique pubkeys.
fn build_its_execute_accounts() -> solana_axelar_its::accounts::Execute {
    solana_axelar_its::accounts::Execute {
        executable: solana_axelar_its::accounts::AxelarExecuteAccounts {
            incoming_message_pda: Pubkey::new_unique(),
            signing_pda: Pubkey::new_unique(),
            gateway_root_pda: Pubkey::new_unique(),
            event_authority: Pubkey::new_unique(),
            axelar_gateway_program: solana_axelar_gateway::ID,
        },
        payer: Pubkey::new_unique(),
        its_root_pda: Pubkey::new_unique(),
        token_manager_pda: Pubkey::new_unique(),
        token_mint: Pubkey::new_unique(),
        token_manager_ata: Pubkey::new_unique(),
        token_program: token_2022::ID,
        associated_token_program: anchor_spl::associated_token::ID,
        system_program: solana_sdk_ids::system_program::ID,
        event_authority: Pubkey::new_unique(),
        program: solana_axelar_its::ID,
    }
}

// ---- Gateway GMP Execute ----

/// In GMP execute, the instruction data contains `payload_without_accounts`.
/// Accounts are only in the transaction (reconstructed on-chain for hash verification).
/// Per-account cost: 33 bytes (tx only).
fn build_gmp_execute_ix(
    n_program_accounts: usize,
    user_payload: &[u8],
    n_payload_accounts: usize,
) -> Instruction {
    let destination_program_id = Pubkey::new_unique();

    let mut accounts = vec![AccountMeta::new(Pubkey::new_unique(), true)]; // payer
    accounts.extend([
        AccountMeta::new(Pubkey::new_unique(), false),               // incoming_message_pda
        AccountMeta::new_readonly(Pubkey::new_unique(), false),      // signing_pda
        AccountMeta::new_readonly(Pubkey::new_unique(), false),      // gateway_root_pda
        AccountMeta::new_readonly(Pubkey::new_unique(), false),      // event_authority
        AccountMeta::new_readonly(solana_axelar_gateway::ID, false), // gateway_program
    ]);
    for _ in 0..n_program_accounts {
        accounts.push(AccountMeta::new(Pubkey::new_unique(), false));
    }
    for _ in 0..n_payload_accounts {
        accounts.push(AccountMeta::new(Pubkey::new_unique(), false));
    }

    let message = typical_cross_chain_message([0u8; 32], &destination_program_id.to_string());

    // GMP ix data: discriminator + Message + payload_without_accounts + encoding_scheme
    let mut data = vec![0u8; 8];
    borsh::to_writer(&mut data, &message).expect("serialize message");
    borsh::to_writer(&mut data, &user_payload.to_vec()).expect("serialize payload");
    data.push(0u8); // encoding_scheme

    Instruction { program_id: destination_program_id, accounts, data }
}

// ---- ITS Execute → InterchainTransfer ----

fn build_execute_interchain_transfer_ix(
    user_data: Option<&[u8]>,
    extra_user_accounts: &[AccountMeta],
) -> Instruction {
    use solana_axelar_its::encoding;

    let destination_address = Pubkey::new_unique();
    let has_data = user_data.is_some();

    let transfer_payload = encoding::InterchainTransfer {
        token_id: [0xAA; 32],
        source_address: b"0xSenderAddressOnEthereum1234".to_vec(),
        destination_address: destination_address.to_bytes().to_vec(),
        amount: 1_000_000u64,
        data: user_data.map(|d| d.to_vec()),
    };

    let hub_message = encoding::HubMessage::ReceiveFromHub {
        source_chain: CHAIN_ETHEREUM.to_owned(),
        message: encoding::Message::InterchainTransfer(transfer_payload),
    };
    let encoded_payload = borsh::to_vec(&hub_message).expect("serialize");
    let payload_hash = solana_sdk::keccak::hashv(&[&encoded_payload]).to_bytes();
    let message = typical_cross_chain_message(payload_hash, &solana_axelar_its::ID.to_string());

    let destination_token_authority = if has_data { Pubkey::new_unique() } else { destination_address };

    let mut accounts = build_its_execute_accounts().to_account_metas(None);
    accounts.extend(
        solana_axelar_its::instructions::execute_interchain_transfer_extra_accounts(
            destination_address,
            destination_token_authority,
            Pubkey::new_unique(), // destination_ata
            Some(has_data),
        ),
    );
    accounts.extend_from_slice(extra_user_accounts);

    Instruction {
        program_id: solana_axelar_its::ID,
        accounts,
        data: solana_axelar_its::instruction::Execute { message, payload: encoded_payload }.data(),
    }
}

// ---- ITS Execute → DeployInterchainToken ----

fn build_execute_deploy_interchain_token_ix() -> Instruction {
    use solana_axelar_its::encoding;

    let hub_message = encoding::HubMessage::ReceiveFromHub {
        source_chain: CHAIN_ETHEREUM.to_owned(),
        message: encoding::Message::DeployInterchainToken(encoding::DeployInterchainToken {
            token_id: [0xBB; 32],
            name: "Wrapped Ether".to_owned(),
            symbol: "WETH".to_owned(),
            decimals: 18,
            minter: Some(Pubkey::new_unique().to_bytes().to_vec()),
        }),
    };
    let encoded_payload = borsh::to_vec(&hub_message).expect("serialize");
    let payload_hash = solana_sdk::keccak::hashv(&[&encoded_payload]).to_bytes();
    let message = typical_cross_chain_message(payload_hash, &solana_axelar_its::ID.to_string());

    let mut accounts = build_its_execute_accounts().to_account_metas(None);
    accounts.extend(
        solana_axelar_its::instructions::execute_deploy_interchain_token_extra_accounts(
            Pubkey::new_unique(), Pubkey::new_unique(), Pubkey::new_unique(),
            Some(Pubkey::new_unique()), Some(Pubkey::new_unique()),
        ),
    );

    Instruction {
        program_id: solana_axelar_its::ID,
        accounts,
        data: solana_axelar_its::instruction::Execute { message, payload: encoded_payload }.data(),
    }
}

// ---- ITS Execute → LinkToken ----

fn build_execute_link_token_ix() -> Instruction {
    use solana_axelar_its::encoding;

    let hub_message = encoding::HubMessage::ReceiveFromHub {
        source_chain: CHAIN_ETHEREUM.to_owned(),
        message: encoding::Message::LinkToken(encoding::LinkToken {
            token_id: [0xCC; 32],
            token_manager_type: 2,
            source_token_address: Pubkey::new_unique().to_bytes().to_vec(),
            destination_token_address: Pubkey::new_unique().to_bytes().to_vec(),
            params: None,
        }),
    };
    let encoded_payload = borsh::to_vec(&hub_message).expect("serialize");
    let payload_hash = solana_sdk::keccak::hashv(&[&encoded_payload]).to_bytes();
    let message = typical_cross_chain_message(payload_hash, &solana_axelar_its::ID.to_string());

    let mut accounts = build_its_execute_accounts().to_account_metas(None);
    accounts.extend(
        solana_axelar_its::instructions::execute_link_token_extra_accounts(
            Some(Pubkey::new_unique()), Some(Pubkey::new_unique()),
        ),
    );

    Instruction {
        program_id: solana_axelar_its::ID,
        accounts,
        data: solana_axelar_its::instruction::Execute { message, payload: encoded_payload }.data(),
    }
}

// ---- ITS Execute → InterchainTransfer with AxelarMessagePayload ----

fn build_axelar_message_payload(user_payload: &[u8], n_accounts: usize) -> Vec<u8> {
    use solana_axelar_gateway::payload::{AxelarMessagePayload, SolanaAccountRepr};

    let accounts: Vec<SolanaAccountRepr> = (0..n_accounts)
        .map(|_| SolanaAccountRepr::from(AccountMeta::new(Pubkey::new_unique(), false)))
        .collect();

    AxelarMessagePayload::new(
        user_payload,
        &accounts,
        solana_axelar_gateway::payload::EncodingScheme::Borsh,
    )
    .encode()
    .expect("encode payload")
}

fn build_its_execute_with_payload(user_payload: &[u8], n_user_accounts: usize) -> Instruction {
    let axelar_payload = build_axelar_message_payload(user_payload, n_user_accounts);
    let user_accounts: Vec<AccountMeta> = (0..n_user_accounts)
        .map(|_| AccountMeta::new(Pubkey::new_unique(), false))
        .collect();
    build_execute_interchain_transfer_ix(Some(&axelar_payload), &user_accounts)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn inbound_transaction_sizes() {
    println!("\n{SEPARATOR}");
    println!("INBOUND TRANSACTION SIZES (legacy, no ALT)");
    println!("Max transaction size: {MAX_TX_SIZE} bytes");
    println!("{SEPARATOR}\n");

    let scenarios: Vec<(&str, TxSizeBreakdown)> = vec![
        ("GMP Execute (base)",
            compute_legacy_tx_size(&build_gmp_execute_ix(0, &[], 0))),
        ("ITS Execute→Transfer (no data)",
            compute_legacy_tx_size(&build_execute_interchain_transfer_ix(None, &[]))),
        ("ITS Execute→Transfer (64B data)",
            compute_legacy_tx_size(&build_execute_interchain_transfer_ix(Some(&[0u8; 64]), &[]))),
        ("ITS Execute→DeployToken",
            compute_legacy_tx_size(&build_execute_deploy_interchain_token_ix())),
        ("ITS Execute→LinkToken",
            compute_legacy_tx_size(&build_execute_link_token_ix())),
    ];

    println!("{:<35} {:>5} {:>4} {:>4} {:>6} {:>9}",
        "Instruction", "Total", "Accs", "Sigs", "IxData", "Remaining");
    println!("{:-<35} {:->5} {:->4} {:->4} {:->6} {:->9}", "", "", "", "", "", "");
    for (name, b) in &scenarios {
        let remaining = MAX_TX_SIZE.saturating_sub(b.total);
        let status = if b.total <= MAX_TX_SIZE { "" } else { " OVERFLOW!" };
        println!("{:<35} {:>5} {:>4} {:>4} {:>6} {:>9}{}",
            name, b.total, b.num_unique_accounts, b.num_signers, b.ix_data_len, remaining, status);
    }

    // With ITS global ALT
    println!("\n--- With ITS global ALT (7 entries, {} bytes net savings) ---\n", ITS_GLOBAL_ALT_SAVINGS);

    let its_scenarios: Vec<(&str, TxSizeBreakdown)> = vec![
        ("ITS Execute→Transfer (no data)",
            compute_legacy_tx_size(&build_execute_interchain_transfer_ix(None, &[]))),
        ("ITS Execute→Transfer (64B data)",
            compute_legacy_tx_size(&build_execute_interchain_transfer_ix(Some(&[0u8; 64]), &[]))),
        ("ITS Execute→Transfer (128B data)",
            compute_legacy_tx_size(&build_execute_interchain_transfer_ix(Some(&[0u8; 128]), &[]))),
        ("ITS Execute→DeployToken",
            compute_legacy_tx_size(&build_execute_deploy_interchain_token_ix())),
        ("ITS Execute→LinkToken",
            compute_legacy_tx_size(&build_execute_link_token_ix())),
    ];

    println!("{:<35} {:>7} {:>9} {:>9} {:>6}",
        "Instruction", "Legacy", "With ALT", "Remaining", "Extra");
    println!("{:-<35} {:->7} {:->9} {:->9} {:->6}", "", "", "", "", "");
    for (name, b) in &its_scenarios {
        let with_alt = b.total.saturating_sub(ITS_GLOBAL_ALT_SAVINGS);
        let remaining = MAX_TX_SIZE.saturating_sub(with_alt);
        println!("{:<35} {:>7} {:>9} {:>9} {:>6}",
            name, b.total, with_alt, remaining, accounts_that_fit(remaining));
    }
}

#[test]
fn protocol_limits() {
    println!("\n{SEPARATOR}");
    println!("PROTOCOL LIMITS: max user payload + accounts");
    println!("{SEPARATOR}");

    // =====================================================================
    // 1. Gateway GMP Execute (no ALT)
    // =====================================================================
    //
    // Instruction data: payload_without_accounts (accounts reconstructed on-chain).
    // Per-account cost: 33 bytes (tx only).
    // Per-byte of payload: 1 byte.

    println!("\n=== 1. Gateway GMP Execute (no ALT) ===");
    println!("Accounts NOT in instruction data (reconstructed on-chain).");
    println!("Per-account cost: 33 bytes | Per-byte of payload: 1 byte\n");

    let gmp_base = compute_legacy_tx_size(&build_gmp_execute_ix(0, &[], 0)).total;
    println!("Base (0 program accounts, empty payload): {} bytes", gmp_base);
    println!("Budget: {} bytes\n", MAX_TX_SIZE.saturating_sub(gmp_base));

    println!("{:>10} {:>12}", "Accounts", "Max payload");
    println!("{:->10} {:->12}", "", "");
    for n_accs in [0, 1, 2, 3, 5, 8, 10, 15, 20] {
        let max_payload = find_max_payload(|p| {
            compute_legacy_tx_size(&build_gmp_execute_ix(0, &vec![0u8; p], n_accs)).total <= MAX_TX_SIZE
        });
        println!("{:>10} {:>12}", n_accs, max_payload);
    }
    println!("\nNote: program-specific accounts + payload accounts all come from this budget at 33 bytes each.");

    // =====================================================================
    // 2. ITS Execute → InterchainTransfer WITHOUT data
    // =====================================================================

    println!("\n=== 2. ITS Execute → InterchainTransfer (no data) ===\n");

    let no_data_legacy = compute_legacy_tx_size(&build_execute_interchain_transfer_ix(None, &[])).total;
    let no_data_l1 = no_data_legacy.saturating_sub(ITS_GLOBAL_ALT_SAVINGS);
    println!("Legacy:        {} / {} bytes ({} remaining)", no_data_legacy, MAX_TX_SIZE, MAX_TX_SIZE - no_data_legacy);
    println!("Level 1 (ALT): {} / {} bytes ({} remaining)", no_data_l1, MAX_TX_SIZE, MAX_TX_SIZE - no_data_l1);
    println!("\nNo user payload or accounts. Fits comfortably.");

    // =====================================================================
    // 3. ITS Execute → DeployInterchainToken / LinkToken
    // =====================================================================

    println!("\n=== 3. ITS Execute → DeployInterchainToken / LinkToken ===\n");

    let deploy_legacy = compute_legacy_tx_size(&build_execute_deploy_interchain_token_ix()).total;
    let deploy_l1 = deploy_legacy.saturating_sub(ITS_GLOBAL_ALT_SAVINGS);
    println!("DeployInterchainToken:");
    println!("  Legacy: {} ({} remaining) | Level 1: {} ({} remaining)",
        deploy_legacy, MAX_TX_SIZE - deploy_legacy, deploy_l1, MAX_TX_SIZE - deploy_l1);

    let link_legacy = compute_legacy_tx_size(&build_execute_link_token_ix()).total;
    let link_l1 = link_legacy.saturating_sub(ITS_GLOBAL_ALT_SAVINGS);
    println!("LinkToken:");
    println!("  Legacy: {} ({} remaining) | Level 1: {} ({} remaining)",
        link_legacy, MAX_TX_SIZE - link_legacy, link_l1, MAX_TX_SIZE - link_l1);

    println!("\nNo variable user payload. Fixed size, fits comfortably with ALT.");

    // =====================================================================
    // 4. ITS Execute → InterchainTransfer WITH data
    // =====================================================================
    //
    // InterchainTransfer.data = full AxelarMessagePayload (accounts + payload).
    // User accounts are in BOTH data and tx remaining_accounts.
    //
    // Per-account cost:
    //   Level 1: 33 (data) + 33 (tx) = 66 bytes
    //   Level 2: 33 (data) + 1 (temp ALT index) = 34 bytes

    println!("\n=== 4. ITS Execute → InterchainTransfer (with data) ===");
    println!("User accounts in BOTH data (AxelarMessagePayload) AND tx remaining_accounts.\n");

    let data_base_legacy = compute_legacy_tx_size(
        &build_execute_interchain_transfer_ix(Some(&build_axelar_message_payload(&[], 0)), &[])
    ).total;
    let data_base_l1 = data_base_legacy.saturating_sub(ITS_GLOBAL_ALT_SAVINGS);

    println!("Base (empty payload, 0 user accounts):");
    println!("  Legacy: {} bytes | Level 1: {} bytes", data_base_legacy, data_base_l1);

    // Level 1
    println!("\n--- Level 1 (global ALT) ---");
    println!("Per-account cost: 66 bytes (33 data + 33 tx)");
    println!("Budget: {} bytes\n", MAX_TX_SIZE.saturating_sub(data_base_l1));

    println!("{:>10} {:>12}", "Accounts", "Max payload");
    println!("{:->10} {:->12}", "", "");
    for n_accs in [0, 1, 2, 3, 4, 5] {
        let max_payload = find_max_payload(|p| {
            let ix = build_its_execute_with_payload(&vec![0u8; p], n_accs);
            compute_legacy_tx_size(&ix).total.saturating_sub(ITS_GLOBAL_ALT_SAVINGS) <= MAX_TX_SIZE
        });
        println!("{:>10} {:>12}", n_accs, max_payload);
    }

    // Level 2
    // Temp ALT: 9 fixed accounts (per-message, per-token, per-transfer) + N user accounts.
    // token_program is now in the global ALT (both Token + Token2022), so not here.
    //   incoming_message_pda(w), signing_pda(r),
    //   token_manager_pda(w), token_mint(w), token_manager_ata(w),
    //   destination(r), destination_token_authority(r), destination_ata(w),
    //   interchain_transfer_execute(r)
    // Writable: 5, Readonly: 4
    // Fixed savings: 9 × 32 - (32 + 1 + 5 + 1 + 4) = 288 - 43 = 245 bytes
    // Per user account in temp ALT: saves 32-1 = 31 bytes vs L1
    const TEMP_ALT_FIXED_SAVINGS: usize = 9 * 32 - (32 + 1 + 5 + 1 + 4); // 245

    let data_base_l2 = data_base_l1.saturating_sub(TEMP_ALT_FIXED_SAVINGS);
    let data_budget_l2 = MAX_TX_SIZE.saturating_sub(data_base_l2);

    println!("\n--- Level 2 (global + temp ALT) ---");
    println!("Per-account cost: 34 bytes (33 data + 1 temp ALT index)");
    println!("Budget: ~{} bytes\n", data_budget_l2);

    println!("{:>10} {:>12}", "Accounts", "Max payload");
    println!("{:->10} {:->12}", "", "");
    for n_accs in [0, 1, 2, 3, 5, 8, 10, 15] {
        let account_cost = n_accs * 34; // 33 data + 1 ALT index
        let max_payload = data_budget_l2.saturating_sub(account_cost);
        println!("{:>10} {:>12}", n_accs, max_payload);
    }
}
