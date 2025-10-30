#![cfg(test)]
#![allow(clippy::str_to_string, clippy::indexing_slicing)]
use anchor_lang::{AccountDeserialize, InstructionData, ToAccountMetas};
use axelar_solana_encoding::hasher::MerkleTree;
use axelar_solana_encoding::hasher::SolanaSyscallHasher;
use axelar_solana_gateway_v2::executable::{ExecutablePayload, ExecutablePayloadEncodingScheme};
use axelar_solana_gateway_v2::seed_prefixes::VALIDATE_MESSAGE_SIGNING_SEED;
use axelar_solana_gateway_v2::IncomingMessage;
use axelar_solana_gateway_v2::ID as GATEWAY_PROGRAM_ID;
use axelar_solana_gateway_v2::{CrossChainId, Message, MessageLeaf};
use axelar_solana_gateway_v2_test_fixtures::{
    approve_message_helper, create_verifier_info, initialize_gateway,
    initialize_payload_verification_session_with_root, setup_test_with_real_signers,
    verify_signature_helper,
};
use axelar_solana_executable::Counter;
use axelar_solana_executable::Payload;
use axelar_solana_executable::ID as MEMO_PROGRAM_ID;
use solana_sdk::msg;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::{
    account::Account,
    instruction::{AccountMeta, Instruction},
    native_token::LAMPORTS_PER_SOL,
    system_program::ID as SYSTEM_PROGRAM_ID,
};
use anchor_lang::AnchorSerialize;
use relayer_discovery_test_fixtures::RelayerDiscoveryTestFixture;

#[test]
#[allow(clippy::too_many_lines)]
#[allow(clippy::non_ascii_literal)]
fn test_execute() {
    let mut fixture = RelayerDiscoveryTestFixture::new();

    // Add the memo program to the Mollusk instance
    fixture.setup.mollusk.add_program(
        &axelar_solana_executable::id(),
        "../../target/deploy/axelar_solana_executable",
        &solana_sdk::bpf_loader_upgradeable::id(),
    );

    // Step 7.1: Init Transaction PDA
    let transaction_pda = relayer_discovery::find_transaction_pda(&axelar_solana_executable::id()).0;
    let init_ix = axelar_solana_executable::instruction::Init {};
    let init_accounts = axelar_solana_executable::accounts::Init {
        relayer_transaction: transaction_pda,
        payer: fixture.setup.payer,
        system_program: SYSTEM_PROGRAM_ID,
    };
    let init_instruction = Instruction {
        program_id: MEMO_PROGRAM_ID,
        accounts: init_accounts.to_account_metas(None),
        data: init_ix.data(),
    };
    let init_accounts = vec![
        (
            transaction_pda,
            Account {
                lamports: 0,
                data: vec![],
                owner: SYSTEM_PROGRAM_ID,
                executable: false,
                rent_epoch: 0,
            },
        ),
        (
            fixture.setup.payer,
            Account {
                lamports: LAMPORTS_PER_SOL,
                data: vec![],
                owner: SYSTEM_PROGRAM_ID,
                executable: false,
                rent_epoch: 0,
            },
        ),
        (
            SYSTEM_PROGRAM_ID,
            Account {
                lamports: 1,
                data: vec![],
                owner: solana_sdk::native_loader::id(),
                executable: true,
                rent_epoch: 0,
            },
        ),
    ];

    let init_result = fixture.setup
        .mollusk
        .process_instruction(&init_instruction, &init_accounts);
    
    assert!(init_result.program_result.is_ok());

    // Step 0: Example payload
    let memo_string = String::from("🐪🐪🐪🐪");
    let storage_id = 123;
    let payload = Payload {
        storage_id,
        memo: memo_string,
    };
    let payload_bytes: Vec<u8> = {
        let mut bytes= Vec::with_capacity(size_of::<Payload>());
        payload.serialize(&mut bytes).unwrap();
        bytes
    };
    let payload_hash: [u8; 32] = solana_program::keccak::hash(&payload_bytes).to_bytes();

    // Step 3: Create message merkle tree
    let message = Message {
        cc_id: CrossChainId {
            chain: "ethereum".to_string(),
            id: "memo_msg_1".to_string(),
        },
        source_address: "0x1234567890123456789012345678901234567890".to_string(),
        destination_chain: "solana".to_string(),
        destination_address: axelar_solana_executable::id().to_string(), // This is crucial!
        payload_hash: payload_hash,
    };

    let result = fixture.approve_and_execute(&message, payload_bytes, init_result.resulting_accounts);

    msg!("{}", format!("{result:?}"));
    assert!(result.is_ok());
}
