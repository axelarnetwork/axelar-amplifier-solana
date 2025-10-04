use crate::u256::U256;
use anchor_lang::prelude::*;

#[event]
pub struct MessageApprovedEvent {
    pub command_id: [u8; 32],
    pub destination_address: Pubkey,
    pub payload_hash: [u8; 32],
    pub source_chain: String,
    pub message_id: String,
    pub source_address: String,
    pub destination_chain: String,
}

#[event]
pub struct MessageExecutedEvent {
    pub command_id: [u8; 32],
    pub destination_address: Pubkey,
    pub payload_hash: [u8; 32],
    pub source_chain: String,
    pub message_id: String,
    pub source_address: String,
    pub destination_chain: String,
}

#[event]
pub struct SignersRotatedEvent {
    pub new_verifier_set_merkle_root: [u8; 32],
    pub epoch: U256,
}

#[event]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CallContractEvent {
    pub sender_key: Pubkey,
    pub payload_hash: [u8; 32],
    pub destination_chain: String,
    pub destination_contract_address: String,
    pub payload: Vec<u8>,
}

#[event]
pub struct OperatorshipTransferedEvent {
    pub new_operator: [u8; 32],
}
