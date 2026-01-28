#![cfg(test)]
#![allow(clippy::str_to_string, clippy::indexing_slicing)]
use anchor_lang::{AccountDeserialize, InstructionData, ToAccountMetas};
use solana_axelar_gateway::IncomingMessage;
use solana_axelar_gateway::ValidateMessageSigner;
use solana_axelar_gateway::ID as GATEWAY_PROGRAM_ID;
use solana_axelar_gateway_test_fixtures::{
    approve_message_helper, create_merklized_messages_from_std, create_signing_verifier_set_leaves,
    initialize_gateway, initialize_payload_verification_session, setup_test_with_real_signers,
    verify_signature_helper,
};
use solana_axelar_memo::Counter;
use solana_axelar_memo::ID as MEMO_PROGRAM_ID;
use solana_axelar_std::{CrossChainId, Message, Messages, Payload, PayloadType};
use solana_sdk::{account::Account, instruction::Instruction, native_token::LAMPORTS_PER_SOL};
use solana_sdk_ids::system_program::ID as SYSTEM_PROGRAM_ID;

#[test]
#[allow(clippy::too_many_lines)]
#[allow(clippy::non_ascii_literal)]
fn execute() {
    // Step 0: Example payload
    let memo_string = "🐪🐪🐪🐪";
    let (counter_pda, _counter_pda_bump) = Counter::find_pda();
    let test_payload = Vec::from(memo_string.as_bytes());
    let test_payload_hash: [u8; 32] = solana_sdk::keccak::hash(test_payload.as_slice()).to_bytes();

    // Step 1: Setup test with real signers
    let (mut setup, secret_key_1, secret_key_2) = setup_test_with_real_signers();

    // Add the memo program to the Mollusk instance
    setup
        .mollusk
        .add_program(&MEMO_PROGRAM_ID, "../../target/deploy/solana_axelar_memo");

    // Step 2: Initialize gateway
    let init_result = initialize_gateway(&setup);

    // Step 3: Create message merkle tree
    let message = Message {
        cc_id: CrossChainId {
            chain: "ethereum".to_string(),
            id: "memo_msg_1".to_string(),
        },
        source_address: "0x1234567890123456789012345678901234567890".to_string(),
        destination_chain: "solana".to_string(),
        destination_address: MEMO_PROGRAM_ID.to_string(), // This is crucial!
        payload_hash: test_payload_hash,
    };

    let messages = vec![message.clone()];

    // Create payload merkle root using std crate approach
    let (_, payload_merkle_root) =
        create_merklized_messages_from_std(setup.domain_separator, &messages);
    let payload_type = PayloadType::ApproveMessages;

    // Step 4: Initialize payload verification session
    let gateway_root_account = init_result
        .get_account(&setup.gateway_root_pda)
        .unwrap()
        .clone();

    let verifier_set_tracker_account = init_result
        .get_account(&setup.verifier_set_tracker_pda)
        .unwrap()
        .clone();

    let (session_result, verification_session_pda) = initialize_payload_verification_session(
        &setup,
        gateway_root_account.clone(),
        verifier_set_tracker_account.clone(),
        payload_merkle_root,
        payload_type,
    );

    let verification_session_account = session_result
        .get_account(&verification_session_pda)
        .unwrap()
        .clone();

    // Step 5: Sign the payload with both signers, verify both signatures on the gateway
    let payload_to_be_signed = Payload::Messages(Messages(messages.clone()));
    let signing_verifier_set_leaves = create_signing_verifier_set_leaves(
        setup.domain_separator,
        &secret_key_1,
        &secret_key_2,
        payload_to_be_signed,
        setup.verifier_set.clone(),
    );

    let verifier_info_1 = signing_verifier_set_leaves[0].clone();

    let verify_result_1 = verify_signature_helper(
        &setup,
        payload_merkle_root,
        verifier_info_1,
        (
            verification_session_pda,
            verification_session_account.clone(),
        ),
        gateway_root_account.clone(),
        (
            setup.verifier_set_tracker_pda,
            verifier_set_tracker_account.clone(),
        ),
    );

    let updated_verification_account_after_first = verify_result_1
        .get_account(&verification_session_pda)
        .unwrap()
        .clone();

    let verifier_info_2 = signing_verifier_set_leaves[1].clone();

    let verify_result_2 = verify_signature_helper(
        &setup,
        payload_merkle_root,
        verifier_info_2,
        (
            verification_session_pda,
            updated_verification_account_after_first.clone(),
        ),
        gateway_root_account.clone(),
        (
            setup.verifier_set_tracker_pda,
            verifier_set_tracker_account.clone(),
        ),
    );

    // Step 6: Approve the message
    let final_gateway_account = verify_result_2
        .get_account(&setup.gateway_root_pda)
        .unwrap()
        .clone();
    let final_verification_session_account = verify_result_2
        .get_account(&verification_session_pda)
        .unwrap()
        .clone();

    let (approve_result, incoming_message_pda) = approve_message_helper(
        &setup,
        &messages,
        (verification_session_pda, final_verification_session_account),
        final_gateway_account,
        0, // position
    );

    assert!(
        !approve_result.program_result.is_err(),
        "Message approval should succeed"
    );

    let incoming_message_account = approve_result
        .get_account(&incoming_message_pda)
        .unwrap()
        .clone();

    let incoming_message =
        IncomingMessage::try_deserialize(&mut incoming_message_account.data.as_slice()).unwrap();

    // Step 7.1: Init Counter PDA
    let init_ix = solana_axelar_memo::instruction::Init {};
    let init_accounts = solana_axelar_memo::accounts::Init {
        counter: counter_pda,
        payer: setup.payer,
        system_program: SYSTEM_PROGRAM_ID,
    };
    let init_instruction = Instruction {
        program_id: MEMO_PROGRAM_ID,
        accounts: init_accounts.to_account_metas(None),
        data: init_ix.data(),
    };
    let init_accounts = vec![
        (
            counter_pda,
            Account {
                lamports: 0,
                data: vec![],
                owner: SYSTEM_PROGRAM_ID,
                executable: false,
                rent_epoch: 0,
            },
        ),
        (
            setup.payer,
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

    let init_result = setup
        .mollusk
        .process_instruction(&init_instruction, &init_accounts);

    assert!(init_result.program_result.is_ok());

    let counter_pda_account = init_result.get_account(&counter_pda).unwrap().clone();

    // Step 7.2: Execute the message
    let message = &messages[0];
    let command_id = message.command_id();

    let signing_pda = ValidateMessageSigner::create_pda(
        &command_id,
        incoming_message.signing_pda_bump,
        &MEMO_PROGRAM_ID,
    )
    .unwrap();

    let (event_authority_pda, _) = solana_axelar_gateway::EVENT_AUTHORITY_AND_BUMP;

    let execute_instruction_data = solana_axelar_memo::instruction::Execute {
        message: message.clone(),
        payload: test_payload,
    }
    .data();

    let execute_accounts = vec![
        (incoming_message_pda, incoming_message_account.clone()),
        (
            signing_pda,
            Account {
                lamports: LAMPORTS_PER_SOL,
                data: vec![],
                owner: GATEWAY_PROGRAM_ID,
                executable: false,
                rent_epoch: 0,
            },
        ),
        (
            GATEWAY_PROGRAM_ID,
            Account {
                lamports: 1,
                data: vec![],
                owner: solana_sdk_ids::bpf_loader_upgradeable::id(),
                executable: true,
                rent_epoch: 0,
            },
        ),
        (
            event_authority_pda,
            Account {
                lamports: 0,
                data: vec![],
                owner: SYSTEM_PROGRAM_ID,
                executable: false,
                rent_epoch: 0,
            },
        ),
        (setup.gateway_root_pda, gateway_root_account.clone()),
        (
            MEMO_PROGRAM_ID,
            Account {
                lamports: 1,
                data: vec![],
                owner: solana_sdk_ids::bpf_loader_upgradeable::id(),
                executable: true,
                rent_epoch: 0,
            },
        ),
        (counter_pda, counter_pda_account.clone()),
    ];

    let execute_ix_accounts = solana_axelar_memo::accounts::Execute {
        executable: solana_axelar_memo::accounts::AxelarExecuteAccounts {
            incoming_message_pda,
            signing_pda,
            gateway_root_pda: setup.gateway_root_pda,
            axelar_gateway_program: GATEWAY_PROGRAM_ID,
            event_authority: event_authority_pda,
        },
        counter: counter_pda,
    }
    .to_account_metas(None);

    let execute_instruction = Instruction {
        program_id: MEMO_PROGRAM_ID,
        accounts: execute_ix_accounts,
        data: execute_instruction_data,
    };

    let execute_result = setup
        .mollusk
        .process_instruction(&execute_instruction, &execute_accounts);

    assert!(
        !execute_result.program_result.is_err(),
        "Execute instruction should succeed: {:?}",
        execute_result.program_result
    );

    let counter_pda_account = execute_result.get_account(&counter_pda).unwrap().clone();

    let counter_data = Counter::try_deserialize(&mut counter_pda_account.data.as_slice()).unwrap();
    assert_eq!(
        counter_data.counter, 1,
        "Counter should be incremented to 1"
    );

    // TODO test event cpi
}
