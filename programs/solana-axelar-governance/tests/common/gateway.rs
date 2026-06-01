#![cfg(test)]
#![allow(dead_code)]

use mollusk_harness::{GatewaySetup, GatewayTestHarness, TestHarness};
use mollusk_svm::result::{InstructionResult, ProgramResult};
use mollusk_svm::Mollusk;
use solana_axelar_gateway::{IncomingMessage, Message};
use solana_axelar_std::CrossChainId;
use solana_sdk::{account::Account, pubkey::Pubkey};

pub struct TestSetup {
    pub mollusk: Mollusk,
    pub harness: GatewayTestHarness,
    pub gateway_root_pda: Pubkey,
    pub verifier_set_tracker_pda: Pubkey,
}

pub fn setup_test_with_real_signers(
) -> (TestSetup, libsecp256k1::SecretKey, libsecp256k1::SecretKey) {
    let harness = GatewayTestHarness::new();
    let secret_key_1 = harness.gateway.signers[0];
    let secret_key_2 = harness.gateway.signers[1];

    let setup = TestSetup {
        mollusk: mollusk_harness::gateway::initialize_gateway_mollusk(),
        gateway_root_pda: harness.gateway.root,
        verifier_set_tracker_pda: harness.gateway.verifier_set_tracker,
        harness,
    };

    (setup, secret_key_1, secret_key_2)
}

pub fn initialize_gateway(setup: &TestSetup) -> InstructionResult {
    success_result(vec![
        (
            setup.gateway_root_pda,
            setup
                .harness
                .get_account(&setup.gateway_root_pda)
                .expect("gateway root should exist"),
        ),
        (
            setup.verifier_set_tracker_pda,
            setup
                .harness
                .get_account(&setup.verifier_set_tracker_pda)
                .expect("verifier set tracker should exist"),
        ),
    ])
}

pub fn create_test_message(
    source_chain: &str,
    message_id: &str,
    destination_address: &str,
    payload_hash: [u8; 32],
) -> Message {
    Message {
        cc_id: CrossChainId {
            chain: source_chain.to_owned(),
            id: message_id.to_owned(),
        },
        source_address: "0xSourceAddress".to_owned(),
        destination_chain: "solana".to_owned(),
        destination_address: destination_address.to_owned(),
        payload_hash,
    }
}

pub fn approve_messages_on_gateway(
    setup: &TestSetup,
    messages: Vec<Message>,
    _gateway_account: Account,
    _verifier_set_tracker_account: Account,
    _secret_key_1: &libsecp256k1::SecretKey,
    _secret_key_2: &libsecp256k1::SecretKey,
) -> Vec<(IncomingMessage, Pubkey, Vec<u8>)> {
    setup
        .harness
        .approve_incoming_messages(&messages)
        .into_iter()
        .map(|approved| (approved.incoming_message, approved.pda, approved.data))
        .collect()
}

fn success_result(resulting_accounts: Vec<(Pubkey, Account)>) -> InstructionResult {
    InstructionResult {
        compute_units_consumed: 0,
        execution_time: 0,
        program_result: ProgramResult::Success,
        raw_result: Ok(()),
        return_data: Vec::new(),
        resulting_accounts,
    }
}
