#![cfg(test)]
//! Transaction size analysis for outbound gateway call_contract.
//!
//! Computes the protocol limit for user payload in `call_contract`.
//!
//! Run with:
//!   cargo test -p solana-axelar-gateway --test test_transaction_sizes -- --nocapture

use std::collections::BTreeSet;

use anchor_lang::{InstructionData, ToAccountMetas};
use solana_sdk::instruction::Instruction;
use solana_sdk::pubkey::Pubkey;

const MAX_TX_SIZE: usize = 1232;

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

/// Build a call_contract instruction (direct signer — user wallet).
///
/// This is the only standalone scenario. When called via CPI from another
/// program, call_contract is embedded inside the caller's instruction
/// and doesn't appear as a top-level transaction instruction.
///
/// Accounts (5 metas, but signing_pda=None deduplicates with program_id):
///   caller, signing_pda(None→program_id placeholder), gateway_root_pda,
///   event_authority, program
fn build_call_contract_ix(payload: &[u8]) -> Instruction {
    let caller = Pubkey::new_unique();
    let gateway_root_pda = solana_axelar_gateway::GatewayConfig::find_pda().0;
    let (gateway_event_authority, _) = solana_axelar_gateway::EVENT_AUTHORITY_AND_BUMP;

    let accounts = solana_axelar_gateway::accounts::CallContract {
        caller,
        signing_pda: None,
        gateway_root_pda,
        event_authority: gateway_event_authority,
        program: solana_axelar_gateway::ID,
    };

    // Mark caller as signer (Anchor's UncheckedAccount doesn't set this,
    // but a direct signer must sign the transaction)
    let mut metas = accounts.to_account_metas(None);
    if let Some(caller_meta) = metas.first_mut() {
        caller_meta.is_signer = true;
    }

    Instruction {
        program_id: solana_axelar_gateway::ID,
        accounts: metas,
        data: solana_axelar_gateway::instruction::CallContract {
            destination_chain: "ethereum".to_owned(),
            destination_contract_address: "0x1234567890abcdef1234567890abcdef12345678".to_owned(),
            payload: payload.to_vec(),
            signing_pda_bump: 0,
        }
        .data(),
    }
}

#[test]
fn call_contract_limits() {
    println!("\n========================================================================");
    println!("OUTBOUND: Gateway call_contract payload limits");
    println!("========================================================================\n");

    let base = compute_legacy_tx_size(&build_call_contract_ix(&[]));
    let max_payload = find_max_payload(|p| {
        compute_legacy_tx_size(&build_call_contract_ix(&vec![0u8; p])) <= MAX_TX_SIZE
    });

    println!("Direct signer (user wallet calling gateway):");
    println!("  Base size (empty payload): {} bytes", base);
    println!("  Max payload:              {} bytes", max_payload);
    println!();

    // ALT analysis
    // Static accounts: gateway_root_pda, event_authority (2 readonly)
    // gateway program = instruction's program_id → must be direct
    // caller = signer → must be direct
    // signing_pda = None → placeholder, deduplicates with program_id
    let alt_savings = 2 * 32 - (32 + 1 + 0 + 1 + 2); // 64 - 36 = 28
    println!("ALT analysis (2 static accounts: gateway_root_pda, event_authority):");
    println!("  Net savings: {} bytes — not worth the complexity", alt_savings);
    println!("  Max payload with ALT: ~{} bytes", max_payload + alt_savings);
    println!();
    println!("Note: when call_contract is called via CPI from another program,");
    println!("it is embedded inside the caller's instruction. The payload budget");
    println!("is then constrained by the outer program's transaction size.");
}
