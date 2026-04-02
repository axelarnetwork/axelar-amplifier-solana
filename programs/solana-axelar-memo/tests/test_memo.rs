#![cfg(test)]
#![allow(clippy::indexing_slicing)]

use anchor_lang::{InstructionData, ToAccountMetas};
use mollusk_harness::{its::ItsTestHarness, GatewaySetup, TestHarness};
use mollusk_svm::result::Check;
use solana_axelar_gateway::executable::{ExecutablePayload, ExecutablePayloadEncodingScheme};
use solana_axelar_memo::{Counter, ID as MEMO_PROGRAM_ID};
use solana_sdk::instruction::{AccountMeta, Instruction};

#[test]
fn send_memo_to_gateway() {
    let mut harness = ItsTestHarness::new();
    harness.ensure_memo_program_initialized();

    let send_memo_ix = solana_axelar_memo::instruction::SendMemo {
        destination_chain: "ethereum".to_owned(),
        destination_address: "0xDestinationAddress".to_owned(),
        memo: "test memo".to_owned(),
    };

    let (signing_pda, _) = solana_axelar_gateway::CallContractSigner::find_pda(&MEMO_PROGRAM_ID);
    let (gateway_event_authority, _) = solana_axelar_gateway::EVENT_AUTHORITY_AND_BUMP;

    let accounts = solana_axelar_memo::accounts::SendMemo {
        memo_program: MEMO_PROGRAM_ID,
        signing_pda,
        gateway_root_pda: harness.gateway().root,
        gateway_event_authority,
        gateway_program: solana_axelar_gateway::ID,
    };

    let ix = Instruction {
        program_id: MEMO_PROGRAM_ID,
        accounts: accounts.to_account_metas(None),
        data: send_memo_ix.data(),
    };

    harness
        .ctx
        .process_and_validate_instruction(&ix, &[Check::success()]);
}

#[test]
fn execute_gmp_message() {
    let mut harness = ItsTestHarness::new();
    harness.ensure_memo_program_initialized();
    harness.ensure_trusted_chain("ethereum");

    let counter_pda = Counter::find_pda().0;

    // Create the payload
    let memo_string = "test memo payload";
    let encoding_scheme = ExecutablePayloadEncodingScheme::AbiEncoding;
    let test_payload = ExecutablePayload::new(
        memo_string.as_bytes(),
        &[AccountMeta::new(counter_pda, false)],
        encoding_scheme,
    );
    let payload_hash: [u8; 32] = test_payload.hash().unwrap();

    // Create and approve the GMP message
    let message = solana_axelar_gateway::Message {
        cc_id: solana_axelar_std::CrossChainId {
            chain: "ethereum".to_owned(),
            id: "memo_msg_1".to_owned(),
        },
        source_address: "0x1234567890123456789012345678901234567890".to_owned(),
        destination_chain: "solana".to_owned(),
        destination_address: MEMO_PROGRAM_ID.to_string(),
        payload_hash,
    };

    harness.ensure_approved_incoming_messages(&[message.clone()]);

    // Build the execute instruction
    let incoming_message_pda =
        solana_axelar_gateway::IncomingMessage::find_pda(&message.command_id()).0;
    let incoming_message = harness
        .get_account_as::<solana_axelar_gateway::IncomingMessage>(&incoming_message_pda)
        .expect("incoming message should exist");

    let signing_pda = solana_axelar_gateway::ValidateMessageSigner::create_pda(
        &message.command_id(),
        incoming_message.signing_pda_bump,
        &MEMO_PROGRAM_ID,
    )
    .unwrap();

    let (event_authority, _) = solana_axelar_gateway::EVENT_AUTHORITY_AND_BUMP;

    let execute_accounts = solana_axelar_memo::accounts::Execute {
        executable: solana_axelar_memo::accounts::AxelarExecuteAccounts {
            incoming_message_pda,
            signing_pda,
            gateway_root_pda: harness.gateway().root,
            axelar_gateway_program: solana_axelar_gateway::ID,
            event_authority,
        },
        counter: counter_pda,
    };

    let ix = Instruction {
        program_id: MEMO_PROGRAM_ID,
        accounts: execute_accounts.to_account_metas(None),
        data: solana_axelar_memo::instruction::Execute {
            message,
            payload: test_payload.payload_without_accounts().to_vec(),
            encoding_scheme,
        }
        .data(),
    };

    harness
        .ctx
        .process_and_validate_instruction(&ix, &[Check::success()]);

    // Verify counter was incremented
    let counter = harness
        .get_account_as::<Counter>(&counter_pda)
        .expect("counter should exist");
    assert_eq!(counter.counter, 1);
}
