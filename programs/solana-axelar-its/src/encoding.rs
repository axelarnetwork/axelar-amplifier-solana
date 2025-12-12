//! Borsh serialization mirrors for `interchain-token-service-std`
//! and `its-borsh-translator` types.
//! These mirrors are necessary because Anchor's Borsh serialization
//! is not compatible with the Borsh serialization used in its-borsh-translator
//! environment. Anchor currently uses borsh v0.10.4 while cosmwasm env
//! requires 1.x.
//! Additionally, Anchor currently requires AnchorSerialize/AnchorDeserialize
//! derives for IDL generation.
//!
//! WARNING: These mirrors must be kept in sync with the cosmwasm contract!
//! https://github.com/axelarnetwork/axelar-amplifier/tree/main/contracts/its-borsh-translator

use anchor_lang::prelude::*;

// Borsh-serializable mirror of interchain_token_service_std::InterchainTransfer
#[derive(Clone, Debug, AnchorSerialize, AnchorDeserialize)]
pub struct InterchainTransfer {
    pub token_id: [u8; 32],
    pub source_address: Vec<u8>,
    pub destination_address: Vec<u8>,
    pub amount: u64,
    pub data: Option<Vec<u8>>,
}

/// Borsh-serializable mirror of interchain_token_service_std::DeployInterchainToken
#[derive(Clone, Debug, AnchorSerialize, AnchorDeserialize)]
pub struct DeployInterchainToken {
    pub token_id: [u8; 32],
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
    pub minter: Option<Vec<u8>>,
}

/// Borsh-serializable mirror of interchain_token_service_std::LinkToken
#[derive(Clone, Debug, AnchorSerialize, AnchorDeserialize)]
pub struct LinkToken {
    pub token_id: [u8; 32],
    pub token_manager_type: u8,
    pub source_token_address: Vec<u8>,
    pub destination_token_address: Vec<u8>,
    pub params: Option<Vec<u8>>,
}

/// Borsh-serializable mirror of interchain_token_service_std::RegisterTokenMetadata
#[derive(Clone, Debug, AnchorSerialize, AnchorDeserialize)]
pub struct RegisterTokenMetadata {
    pub decimals: u8,
    pub token_address: Vec<u8>,
}

/// Borsh-serializable mirror of interchain_token_service_std::Message
/// Note: Borsh enums automatically serialize with a discriminant byte
#[derive(Clone, Debug, AnchorSerialize, AnchorDeserialize)]
pub enum Message {
    InterchainTransfer(InterchainTransfer),
    DeployInterchainToken(DeployInterchainToken),
    LinkToken(LinkToken),
}

/// Borsh-serializable mirror of interchain_token_service_std::HubMessage
#[derive(Clone, Debug, AnchorSerialize, AnchorDeserialize)]
pub enum HubMessage {
    SendToHub {
        destination_chain: String,
        message: Message,
    },
    ReceiveFromHub {
        source_chain: String,
        message: Message,
    },
    RegisterTokenMetadata(RegisterTokenMetadata),
}
