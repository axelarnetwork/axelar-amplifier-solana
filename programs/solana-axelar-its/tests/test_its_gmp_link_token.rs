#![cfg(test)]
#![allow(clippy::indexing_slicing)]

use anchor_lang::prelude::borsh;
use anchor_lang::{prelude::AccountMeta, InstructionData, ToAccountMetas};
use anchor_spl::{
    associated_token::get_associated_token_address_with_program_id, token_2022::spl_token_2022,
};
use mollusk_harness::{ItsTestHarness, TestHarness};
use mollusk_svm::result::Check;
use mollusk_test_utils::get_event_authority_and_program_accounts;
use rand::Rng;
use solana_axelar_its::{encoding, state::TokenManager, utils::interchain_token_id, ItsError};
use solana_sdk::{instruction::Instruction, pubkey::Pubkey};

/// Constructs and executes a GMP LinkToken message through the Execute instruction.
/// Unlike `execute_hub_message`, this allows specifying a custom `token_mint`
/// (needed because link token uses an external SPL mint, not a PDA-derived one).
fn execute_gmp_link_token(
    harness: &ItsTestHarness,
    source_chain: &str,
    token_id: [u8; 32],
    token_mint: Pubkey,
    payload: encoding::LinkToken,
    extra_accounts: Vec<AccountMeta>,
    checks: &[Check],
) -> mollusk_svm::result::InstructionResult {
    let hub_message = encoding::HubMessage::ReceiveFromHub {
        source_chain: source_chain.to_owned(),
        message: encoding::Message::LinkToken(payload),
    };

    let encoded_payload = borsh::to_vec(&hub_message).expect("payload should serialize");
    let payload_hash = solana_sdk::keccak::hashv(&[&encoded_payload]).to_bytes();

    let rand_message_id: String = rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(32)
        .map(char::from)
        .collect();

    let its = harness.get_its_root();

    let message = solana_axelar_gateway::Message {
        cc_id: solana_axelar_std::CrossChainId {
            chain: source_chain.to_owned(),
            id: rand_message_id,
        },
        source_address: its.its_hub_address,
        destination_chain: "solana".to_owned(),
        destination_address: solana_axelar_its::ID.to_string(),
        payload_hash,
    };

    harness.ensure_approved_incoming_messages(&[message.clone()]);

    let incoming_message_pda =
        solana_axelar_gateway::IncomingMessage::find_pda(&message.command_id()).0;
    let incoming_message = harness
        .get_account_as::<solana_axelar_gateway::IncomingMessage>(&incoming_message_pda)
        .expect("incoming message should exist");

    let token_manager_pda = TokenManager::find_pda(token_id, harness.its_root).0;
    let token_manager_ata = get_associated_token_address_with_program_id(
        &token_manager_pda,
        &token_mint,
        &spl_token_2022::ID,
    );

    let executable = solana_axelar_its::accounts::AxelarExecuteAccounts {
        incoming_message_pda,
        signing_pda: solana_axelar_gateway::ValidateMessageSigner::create_pda(
            &message.command_id(),
            incoming_message.signing_pda_bump,
            &solana_axelar_its::ID,
        )
        .expect("valid signing PDA"),
        gateway_root_pda: harness.gateway.root,
        event_authority: get_event_authority_and_program_accounts(&solana_axelar_gateway::ID).0,
        axelar_gateway_program: solana_axelar_gateway::ID,
    };

    let mut accounts = solana_axelar_its::accounts::Execute {
        executable,
        payer: harness.payer,
        system_program: solana_sdk_ids::system_program::ID,
        its_root_pda: harness.its_root,
        token_mint,
        token_manager_pda,
        token_manager_ata,
        token_program: spl_token_2022::ID,
        associated_token_program: anchor_spl::associated_token::ID,
        event_authority: get_event_authority_and_program_accounts(&solana_axelar_its::ID).0,
        program: solana_axelar_its::ID,
    }
    .to_account_metas(None);
    accounts.extend(extra_accounts);

    let ix = Instruction {
        program_id: solana_axelar_its::ID,
        accounts,
        data: solana_axelar_its::instruction::Execute {
            message,
            payload: encoded_payload,
        }
        .data(),
    };

    harness
        .ctx
        .process_and_validate_instruction_chain(&[(&ix, checks)])
}

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

    execute_gmp_link_token(
        &harness,
        "ethereum",
        token_id,
        token_mint,
        payload,
        vec![],
        &[Check::success()],
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

    execute_gmp_link_token(
        &harness,
        "ethereum",
        token_id,
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

    execute_gmp_link_token(
        &harness,
        "ethereum",
        token_id,
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

    execute_gmp_link_token(
        &harness,
        "ethereum",
        token_id, // accounts derived from this
        token_mint,
        payload,
        vec![],
        &[Check::err(
            anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::ConstraintSeeds).into(),
        )],
    );
}
