#![cfg(test)]
//! Transaction size analysis for outbound ITS InterchainTransfer.
//!
//! Computes how much space users have for the `data` payload when
//! sending tokens cross-chain via InterchainTransfer.
//!
//! Two scenarios:
//!   - Without a shared ALT (users manage everything themselves)
//!   - With a shared ALT provided by the protocol (static ITS + gateway accounts)
//!
//! Run with:
//!   cargo test -p solana-axelar-its --test test_transaction_sizes_outbound -- --nocapture

use std::collections::BTreeSet;

use anchor_spl::token_2022;
use solana_sdk::instruction::Instruction;
use solana_sdk::message::v0;
use solana_sdk::message::AddressLookupTableAccount;
use solana_sdk::pubkey::Pubkey;

const MAX_TX_SIZE: usize = 1232;

// ---------------------------------------------------------------------------
// Size calculation
// ---------------------------------------------------------------------------

fn compact_u16_len(val: usize) -> usize {
    if val < 0x80 { 1 } else if val < 0x4000 { 2 } else { 3 }
}

fn compute_legacy_tx_size(ix: &Instruction) -> usize {
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

    compact_u16_len(num_signers) + num_signers * 64
        + 3
        + compact_u16_len(num_unique) + num_unique * 32
        + 32
        + compact_u16_len(1) + 1
        + compact_u16_len(num_ix_accounts) + num_ix_accounts
        + compact_u16_len(data_len) + data_len
}

fn find_max_payload(fits: impl Fn(usize) -> bool) -> usize {
    let mut lo = 0usize;
    let mut hi = MAX_TX_SIZE;
    while lo < hi {
        let mid = (lo + hi + 1) / 2;
        if fits(mid) { lo = mid; } else { hi = mid - 1; }
    }
    lo
}

/// Two compute-budget instructions (matching what most clients prepend and
/// what the relayer's helpers also prepend on outbound builders).
fn compute_budget_ixs() -> [Instruction; 2] {
    let cb = solana_sdk_ids::compute_budget::ID;
    let mut price_data = vec![3u8];
    price_data.extend_from_slice(&1u64.to_le_bytes());
    let mut limit_data = vec![2u8];
    limit_data.extend_from_slice(&200_000u32.to_le_bytes());
    [
        Instruction { program_id: cb, accounts: vec![], data: price_data },
        Instruction { program_id: cb, accounts: vec![], data: limit_data },
    ]
}

/// Authoritative size: compile a v0 message with the 2 compute-budget
/// instructions and the given ALTs, walk the structure to compute the
/// exact serialized tx size.
fn compute_v0_tx_size(ixs: &[Instruction], alts: &[AddressLookupTableAccount]) -> usize {
    let payer = ixs
        .iter()
        .flat_map(|ix| ix.accounts.iter())
        .find(|m| m.is_signer)
        .map(|m| m.pubkey)
        .unwrap_or_else(Pubkey::new_unique);
    let dummy_hash = solana_sdk::hash::Hash::default();
    let cb = compute_budget_ixs();
    let mut all_ixs: Vec<Instruction> = cb.into_iter().collect();
    all_ixs.extend_from_slice(ixs);
    let msg = v0::Message::try_compile(&payer, &all_ixs, alts, dummy_hash)
        .expect("compile v0 message");
    let num_sigs = msg.header.num_required_signatures as usize;
    let num_keys = msg.account_keys.len();
    let num_ixs = msg.instructions.len();
    let num_alts = msg.address_table_lookups.len();
    let mut size = compact_u16_len(num_sigs) + num_sigs * 64
        + 1
        + 3
        + compact_u16_len(num_keys) + num_keys * 32
        + 32
        + compact_u16_len(num_ixs);
    for ix in &msg.instructions {
        size += 1
            + compact_u16_len(ix.accounts.len()) + ix.accounts.len()
            + compact_u16_len(ix.data.len()) + ix.data.len();
    }
    size += compact_u16_len(num_alts);
    for alt in &msg.address_table_lookups {
        size += 32
            + compact_u16_len(alt.writable_indexes.len()) + alt.writable_indexes.len()
            + compact_u16_len(alt.readonly_indexes.len()) + alt.readonly_indexes.len();
    }
    size
}

/// Build the protocol-provided shared ALT for outbound interchain_transfer.
///
/// The ALT holds accounts that are stable across users/tokens:
///   gateway_root_pda, gateway_event_authority, gateway_program,
///   call_contract_signing_pda, gas_treasury, gas_service, gas_event_authority,
///   its_root_pda, system_program, event_authority(ITS), token_program.
///
/// This helper extracts those positions directly from the built instruction
/// (the order is fixed by `accounts::InterchainTransfer::to_account_metas`).
/// `anchor_spl::token::ID` is also appended so the ALT works for both Token
/// and Token-2022 mints — but only one of them will appear in any given tx,
/// so v0 compile won't reference it for a Token-2022 tx.
fn build_outbound_shared_alt_from_ix(ix: &Instruction) -> AddressLookupTableAccount {
    // Slot indices in `accounts::InterchainTransfer` that are static:
    //   0 payer (signer)        — skip
    //   1 authority (signer)    — skip
    //   2 gateway_root_pda      ← static
    //   3 gateway_event_authority ← static
    //   4 gateway_program       ← static
    //   5 call_contract_signing_pda ← static
    //   6 gas_treasury (writable) ← static
    //   7 gas_service           ← static
    //   8 gas_event_authority   ← static
    //   9 its_root_pda          ← static
    //  10 token_manager_pda     — per-token, skip
    //  11 token_program         ← static
    //  12 token_mint            — per-token, skip
    //  13 authority_token_account — per-user, skip
    //  14 token_manager_ata     — per-token, skip
    //  15 system_program        ← static
    //  16 event_authority (ITS) ← static
    //  17 program (ITS)         — instruction program_id, skip
    let static_indices = [2, 3, 4, 5, 6, 7, 8, 9, 11, 15, 16];
    let mut addrs: Vec<Pubkey> = static_indices
        .iter()
        .filter_map(|i| ix.accounts.get(*i).map(|a| a.pubkey))
        .collect();
    addrs.push(anchor_spl::token::ID); // included for non-Token-2022 tokens
    let _ = BTreeSet::<Pubkey>::new();
    AddressLookupTableAccount { key: Pubkey::new_unique(), addresses: addrs }
}

// ---------------------------------------------------------------------------
// Instruction builders
// ---------------------------------------------------------------------------

fn build_interchain_transfer(data: Option<Vec<u8>>) -> Instruction {
    let (ix, _) = solana_axelar_its::instructions::make_interchain_transfer_instruction(
        [0xAA; 32],                // token_id
        1_000_000,                 // amount
        token_2022::ID,            // token_program
        Pubkey::new_unique(),      // payer
        Pubkey::new_unique(),      // authority
        "ethereum".to_owned(),     // destination_chain
        vec![0u8; 20],            // destination_address (EVM 20 bytes)
        10_000,                    // gas_value
        None,                      // caller_program_id
        None,                      // caller_pda_seeds
        data,
    );
    ix
}

fn build_interchain_transfer_cpi(data: Option<Vec<u8>>) -> Instruction {
    let (ix, _) = solana_axelar_its::instructions::make_interchain_transfer_instruction(
        [0xAA; 32],
        1_000_000,
        token_2022::ID,
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        "ethereum".to_owned(),
        vec![0u8; 20],
        10_000,
        Some(Pubkey::new_unique()),                              // caller_program_id
        Some(vec![b"seed1".to_vec(), b"seed2".to_vec()]),        // caller_pda_seeds (2 seeds)
        data,
    );
    ix
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn outbound_interchain_transfer_limits() {
    println!("\n========================================================================");
    println!("OUTBOUND: ITS InterchainTransfer payload limits");
    println!("========================================================================\n");

    // --- User wallet (direct signer, no CPI) ---

    let base_no_data = compute_legacy_tx_size(&build_interchain_transfer(None));
    let max_data = find_max_payload(|p| {
        compute_legacy_tx_size(&build_interchain_transfer(Some(vec![0u8; p]))) <= MAX_TX_SIZE
    });

    println!("User wallet (direct signer, no CPI):");
    println!("  Base size (no data): {} bytes", base_no_data);
    println!("  Max `data` payload:  {} bytes\n", max_data);

    // --- CPI caller (program_id + 2 seeds) ---

    let cpi_base = compute_legacy_tx_size(&build_interchain_transfer_cpi(None));
    let cpi_max_data = find_max_payload(|p| {
        compute_legacy_tx_size(&build_interchain_transfer_cpi(Some(vec![0u8; p]))) <= MAX_TX_SIZE
    });

    println!("CPI caller (program_id + 2 seeds):");
    println!("  Base size (no data): {} bytes", cpi_base);
    println!("  Max `data` payload:  {} bytes\n", cpi_max_data);

    // --- ALT analysis ---
    //
    // InterchainTransfer accounts (18 total):
    //   Signers (must be direct):
    //     - payer (signer, mut)
    //     - authority (signer)
    //   Per-token (vary by token):
    //     - token_manager_pda
    //     - token_mint
    //     - authority_token_account (user's ATA)
    //     - token_manager_ata
    //   Static / global (candidates for shared ALT):
    //     - gateway_root_pda
    //     - gateway_event_authority
    //     - gateway_program
    //     - call_contract_signing_pda
    //     - gas_treasury (mut!)
    //     - gas_service
    //     - gas_event_authority
    //     - its_root_pda
    //     - system_program
    //     - event_authority (ITS)
    //     - spl_token (Token program)        — both included so ALT works
    //     - spl_token_2022 (Token2022)       — regardless of token standard
    //   program (ITS) = instruction's program_id, must be direct
    //
    // Shared ALT candidates: 12 accounts

    let alt_entries = 12;
    let alt_writable = 1; // gas_treasury
    let alt_readonly = alt_entries - alt_writable;
    let alt_overhead = 32 + 1 + alt_writable + 1 + alt_readonly; // 32 + 1 + 1 + 1 + 11 = 46
    let alt_savings = alt_entries * 32 - alt_overhead; // 384 - 46 = 338

    let max_data_with_alt = max_data + alt_savings;
    let cpi_max_data_with_alt = cpi_max_data + alt_savings;

    println!("--- With shared ALT (protocol-provided) ---");
    println!("ALT entries (12 static accounts):");
    println!("  gateway_root_pda, gateway_event_authority, gateway_program,");
    println!("  call_contract_signing_pda, gas_treasury, gas_service, gas_event_authority,");
    println!("  its_root_pda, system_program, event_authority (ITS),");
    println!("  spl_token, spl_token_2022");
    println!("ALT net savings: {} bytes\n", alt_savings);

    println!("User wallet + shared ALT:");
    println!("  Max `data` payload: ~{} bytes\n", max_data_with_alt);

    println!("CPI caller + shared ALT:");
    println!("  Max `data` payload: ~{} bytes\n", cpi_max_data_with_alt);

    println!("--- Summary ---");
    println!("{:<30} {:>12} {:>15}", "Scenario", "No ALT", "With shared ALT");
    println!("{:-<30} {:->12} {:->15}", "", "", "");
    println!("{:<30} {:>12} {:>15}", "User wallet", format!("{} bytes", max_data), format!("~{} bytes", max_data_with_alt));
    println!("{:<30} {:>12} {:>15}", "CPI caller (2 seeds)", format!("{} bytes", cpi_max_data), format!("~{} bytes", cpi_max_data_with_alt));

    println!("\nNote: the shared ALT is optional. Users can manage their own ALTs,");
    println!("or use the protocol-provided ALT with these 12 accounts.");
    println!("The ALT saves {} bytes — meaningful for data-heavy transfers.", alt_savings);
}

/// Authoritative measurement (v0 + 2 compute-budget instructions). Use
/// these numbers when documenting protocol limits.
#[test]
fn outbound_realistic_sizes() {
    println!("\n========================================================================");
    println!("OUTBOUND: ITS InterchainTransfer (v0 + compute-budget)");
    println!("========================================================================\n");

    // -- User wallet, no ALT
    let base_no_data = compute_v0_tx_size(&[build_interchain_transfer(None)], &[]);
    let max_data = find_max_payload(|p| {
        compute_v0_tx_size(&[build_interchain_transfer(Some(vec![0u8; p]))], &[]) <= MAX_TX_SIZE
    });
    println!("User wallet, no ALT:");
    println!("  Base size (no data): {} bytes", base_no_data);
    println!("  Max `data` payload:  {} bytes\n", max_data);

    // -- CPI caller (program_id + 2 seeds), no ALT
    let cpi_base = compute_v0_tx_size(&[build_interchain_transfer_cpi(None)], &[]);
    let cpi_max_data = find_max_payload(|p| {
        compute_v0_tx_size(&[build_interchain_transfer_cpi(Some(vec![0u8; p]))], &[]) <= MAX_TX_SIZE
    });
    println!("CPI caller (program_id + 2 seeds), no ALT:");
    println!("  Base size (no data): {} bytes", cpi_base);
    println!("  Max `data` payload:  {} bytes\n", cpi_max_data);

    // -- User wallet, with shared ALT
    let ref_ix = build_interchain_transfer(None);
    let alt = build_outbound_shared_alt_from_ix(&ref_ix);
    let max_data_alt = find_max_payload(|p| {
        let ix = build_interchain_transfer(Some(vec![0u8; p]));
        let alt = build_outbound_shared_alt_from_ix(&ix);
        compute_v0_tx_size(std::slice::from_ref(&ix), std::slice::from_ref(&alt)) <= MAX_TX_SIZE
    });
    println!("User wallet, shared ALT (entries: {}):", alt.addresses.len());
    println!("  Max `data` payload:  {} bytes\n", max_data_alt);

    // -- CPI caller, with shared ALT
    let cpi_max_data_alt = find_max_payload(|p| {
        let ix = build_interchain_transfer_cpi(Some(vec![0u8; p]));
        let alt = build_outbound_shared_alt_from_ix(&ix);
        compute_v0_tx_size(std::slice::from_ref(&ix), std::slice::from_ref(&alt)) <= MAX_TX_SIZE
    });
    println!("CPI caller, shared ALT:");
    println!("  Max `data` payload:  {} bytes\n", cpi_max_data_alt);

    println!("--- Summary (production-realistic, with compute-budget) ---");
    println!("{:<30} {:>12} {:>15}", "Scenario", "No ALT", "Shared ALT");
    println!("{:-<30} {:->12} {:->15}", "", "", "");
    println!("{:<30} {:>12} {:>15}", "User wallet",
        format!("{} B", max_data), format!("{} B", max_data_alt));
    println!("{:<30} {:>12} {:>15}", "CPI caller (2 seeds)",
        format!("{} B", cpi_max_data), format!("{} B", cpi_max_data_alt));
}
