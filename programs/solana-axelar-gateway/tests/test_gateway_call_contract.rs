#![cfg(test)]
#![allow(clippy::indexing_slicing)]

mod helpers;

use mollusk_harness::{GatewayTestHarness, TestHarness};
use solana_sdk::pubkey::Pubkey;

#[test]
fn call_contract_from_program() {
    let harness = GatewayTestHarness::new();

    let memo_program_id = Pubkey::new_unique();

    // Create executable account for the memo program
    let (signing_pda, _) = solana_axelar_gateway::CallContractSigner::find_pda(&memo_program_id);

    // Store the memo program as executable
    harness.ctx.account_store.borrow_mut().insert(
        memo_program_id,
        solana_sdk::account::Account {
            lamports: 1,
            data: vec![],
            owner: solana_sdk::native_loader::id(),
            executable: true,
            rent_epoch: 0,
        },
    );

    // Store the signing PDA owned by the memo program
    harness.ctx.account_store.borrow_mut().insert(
        signing_pda,
        solana_sdk::account::Account {
            lamports: 0,
            data: vec![],
            owner: memo_program_id,
            executable: false,
            rent_epoch: 0,
        },
    );

    harness.call_contract(
        memo_program_id,
        "ethereum".to_owned(),
        "0xdeadbeef".to_owned(),
        b"memo test".to_vec(),
    );
}

#[test]
fn call_contract_direct_signer() {
    let harness = GatewayTestHarness::new();

    let direct_signer = Pubkey::new_unique();
    harness.ensure_account_exists_with_lamports(
        direct_signer,
        solana_sdk::native_token::LAMPORTS_PER_SOL,
    );

    harness.call_contract(
        direct_signer,
        "ethereum".to_owned(),
        "0xDestinationContract".to_owned(),
        b"Hello from Solana!".to_vec(),
    );
}
