#![cfg(test)]
#![allow(clippy::indexing_slicing)]

use mollusk_harness::{ItsTestHarness, TestHarness};
use mollusk_svm::result::Check;
use solana_axelar_its::{
    encoding,
    state::TokenManager,
    utils::interchain_token_id,
    ItsError,
};
use solana_sdk::pubkey::Pubkey;

#[test]
fn execute_link_token_success() {
    let mut harness = ItsTestHarness::new();
    harness.ensure_trusted_chain("ethereum");

    let mint_authority = harness.get_new_wallet();
    let token_mint = harness.create_spl_token_mint(mint_authority, 9, None);

    let salt = [1u8; 32];
    let token_id = interchain_token_id(&harness.payer, &salt);

    let payload = encoding::LinkToken {
        token_id,
        token_manager_type: 1, // LockUnlock
        source_token_address: token_mint.to_bytes().to_vec(),
        destination_token_address: token_mint.to_bytes().to_vec(),
        params: None,
    };

    harness.execute_gmp_link_token(
        token_id,
        "ethereum",
        token_mint,
        payload,
        vec![],
    );

    // Verify token manager was created
    let token_manager_pda = TokenManager::find_pda(token_id, harness.its_root).0;
    let tm: TokenManager = harness
        .get_account_as(&token_manager_pda)
        .expect("token manager should exist");
    assert_eq!(tm.token_address, token_mint);
}

#[test]
fn reject_execute_link_token_with_invalid_token_manager_type() {
    let mut harness = ItsTestHarness::new();
    harness.ensure_trusted_chain("ethereum");

    let mint_authority = harness.get_new_wallet();
    let token_mint = harness.create_spl_token_mint(mint_authority, 9, None);

    let salt = [2u8; 32];
    let token_id = interchain_token_id(&harness.payer, &salt);

    let payload = encoding::LinkToken {
        token_id,
        token_manager_type: 255, // invalid type
        source_token_address: token_mint.to_bytes().to_vec(),
        destination_token_address: token_mint.to_bytes().to_vec(),
        params: None,
    };

    harness.execute_gmp_link_token_with_checks(
        token_id,
        "ethereum",
        token_mint,
        payload,
        vec![],
        &[Check::err(ItsError::InvalidInstructionData.into())],
    );
}

#[test]
fn reject_execute_link_token_with_invalid_destination_token_address() {
    let mut harness = ItsTestHarness::new();
    harness.ensure_trusted_chain("ethereum");

    let mint_authority = harness.get_new_wallet();
    let token_mint = harness.create_spl_token_mint(mint_authority, 9, None);

    let salt = [3u8; 32];
    let token_id = interchain_token_id(&harness.payer, &salt);

    // Use a different address in payload than the actual mint
    let wrong_mint = Pubkey::new_unique();

    let payload = encoding::LinkToken {
        token_id,
        token_manager_type: 1, // LockUnlock
        source_token_address: token_mint.to_bytes().to_vec(),
        destination_token_address: wrong_mint.to_bytes().to_vec(), // mismatch
        params: None,
    };

    harness.execute_gmp_link_token_with_checks(
        token_id,
        "ethereum",
        token_mint,
        payload,
        vec![],
        &[Check::err(ItsError::InvalidTokenMint.into())],
    );
}

#[test]
fn reject_execute_link_token_with_invalid_token_id() {
    let mut harness = ItsTestHarness::new();
    harness.ensure_trusted_chain("ethereum");

    let mint_authority = harness.get_new_wallet();
    let token_mint = harness.create_spl_token_mint(mint_authority, 9, None);

    let salt = [4u8; 32];
    let token_id = interchain_token_id(&harness.payer, &salt);
    let invalid_token_id = [99u8; 32];

    // Payload uses invalid_token_id but accounts use token_id
    let payload = encoding::LinkToken {
        token_id: invalid_token_id, // mismatch with account derivation
        token_manager_type: 1,
        source_token_address: token_mint.to_bytes().to_vec(),
        destination_token_address: token_mint.to_bytes().to_vec(),
        params: None,
    };

    harness.execute_gmp_link_token_with_checks(
        token_id, // accounts derived from this
        "ethereum",
        token_mint,
        payload,
        vec![],
        &[Check::err(
            anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::ConstraintSeeds).into(),
        )],
    );
}
