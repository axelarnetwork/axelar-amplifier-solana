#![allow(clippy::panic, dead_code, unreachable_pub, unused_imports)]

use std::collections::BTreeMap;

use anchor_lang::prelude::AnchorDeserialize;
use mollusk_harness::gateway::create_verifier_info;
use mollusk_harness::GatewayTestHarness;
use solana_axelar_gateway::GatewayError;
use solana_axelar_std::{
    Message, Payload, PayloadType, PublicKey, SigningVerifierSetInfo, VerifierSet,
};

/// Helper to convert gateway errors to ProgramError for Check::err.
pub fn gateway_err(e: GatewayError) -> solana_sdk::program_error::ProgramError {
    let anchor_err: anchor_lang::error::Error = e.into();
    anchor_err.into()
}

pub fn default_messages() -> Vec<Message> {
    vec![
        Message {
            cc_id: solana_axelar_std::CrossChainId {
                chain: "ethereum".to_owned(),
                id: "msg_1".to_owned(),
            },
            source_address: "0xSourceAddress".to_owned(),
            destination_chain: "solana".to_owned(),
            destination_address: "DNHKNbf4JWJNnquuWJuNUSFGsXbDYs1sPR1ZvVhah827".to_owned(),
            payload_hash: [1u8; 32],
        },
        Message {
            cc_id: solana_axelar_std::CrossChainId {
                chain: "ethereum".to_owned(),
                id: "msg_2".to_owned(),
            },
            source_address: "0xSourceAddress".to_owned(),
            destination_chain: "solana".to_owned(),
            destination_address: "8q49wyQjNrSEZf5A8h6jR7dwLnDxdnURftv89FWLWMGK".to_owned(),
            payload_hash: [2u8; 32],
        },
    ]
}

pub fn fake_messages() -> Vec<Message> {
    vec![
        Message {
            cc_id: solana_axelar_std::CrossChainId {
                chain: "ethereum".to_owned(),
                id: "fake msg_1".to_owned(),
            },
            source_address: "0xSourceAddress".to_owned(),
            destination_chain: "solana".to_owned(),
            destination_address: "DNHKNbf4JWJNnquuWJuNUSFGsXbDYs1sPR1ZvVhah827".to_owned(),
            payload_hash: [1u8; 32],
        },
        Message {
            cc_id: solana_axelar_std::CrossChainId {
                chain: "ethereum".to_owned(),
                id: "fake msg_2".to_owned(),
            },
            source_address: "0xSourceAddress".to_owned(),
            destination_chain: "solana".to_owned(),
            destination_address: "8q49wyQjNrSEZf5A8h6jR7dwLnDxdnURftv89FWLWMGK".to_owned(),
            payload_hash: [2u8; 32],
        },
    ]
}

/// Compute the payload merkle root and merklized messages from messages + domain separator.
pub fn create_merklized_messages(
    domain_separator: [u8; 32],
    messages: &[Message],
) -> (Vec<solana_axelar_std::MerklizedMessage>, [u8; 32]) {
    let dummy_pubkey = PublicKey([1u8; 33]);
    let mut signers = BTreeMap::new();
    signers.insert(dummy_pubkey, 1u128);

    let verifier_set = VerifierSet {
        nonce: 0,
        signers,
        quorum: 1,
    };
    let signatures = BTreeMap::new();

    let payload = Payload::Messages(solana_axelar_std::Messages(messages.to_vec()));
    let encoded = solana_axelar_std::execute_data::encode(
        &verifier_set,
        &signatures,
        domain_separator,
        payload,
    )
    .expect("encoding should succeed");

    let execute_data = solana_axelar_std::execute_data::ExecuteData::try_from_slice(&encoded)
        .expect("deserialization should succeed");

    match execute_data.payload_items {
        solana_axelar_std::execute_data::MerklizedPayload::NewMessages { messages } => {
            (messages, execute_data.payload_merkle_root)
        }
        solana_axelar_std::execute_data::MerklizedPayload::VerifierSetRotation { .. } => {
            panic!("expected NewMessages payload, got VerifierSetRotation")
        }
    }
}

/// Build `SigningVerifierSetInfo` for each harness signer for the given payload root and type.
pub fn build_verifier_infos(
    harness: &GatewayTestHarness,
    payload_merkle_root: [u8; 32],
    payload_type: PayloadType,
) -> Vec<SigningVerifierSetInfo> {
    harness
        .gateway
        .signers
        .iter()
        .zip(harness.gateway.verifier_set_leaves.iter())
        .enumerate()
        .map(|(idx, (sk, leaf))| {
            create_verifier_info(
                sk,
                payload_merkle_root,
                leaf,
                idx,
                &harness.gateway.verifier_merkle_tree,
                payload_type,
            )
        })
        .collect()
}

/// Compute the payload merkle root for a new verifier set (rotation hash).
pub fn compute_new_verifier_set_hash(
    domain_separator: [u8; 32],
    new_verifier_set: &VerifierSet,
) -> [u8; 32] {
    solana_axelar_std::execute_data::hash_payload::<solana_axelar_std::hasher::Hasher>(
        &domain_separator,
        Payload::NewVerifierSet(new_verifier_set.clone()),
    )
    .expect("hash_payload should succeed")
}
