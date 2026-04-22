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
