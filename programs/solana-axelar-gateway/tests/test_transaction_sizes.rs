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
use solana_sdk::message::v0;
use solana_sdk::message::AddressLookupTableAccount;
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

#[test]
fn call_contract_realistic_sizes() {
    println!("\n========================================================================");
    println!("OUTBOUND: call_contract (v0 + 2 compute-budget ixs)");
    println!("========================================================================\n");

    let base = compute_v0_tx_size(&[build_call_contract_ix(&[])], &[]);
    let max_payload = find_max_payload(|p| {
        compute_v0_tx_size(&[build_call_contract_ix(&vec![0u8; p])], &[]) <= MAX_TX_SIZE
    });
    println!("Direct signer:");
    println!("  Base size (empty payload): {} bytes", base);
    println!("  Max payload:              {} bytes", max_payload);
}
