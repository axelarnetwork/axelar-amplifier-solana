//! Borsh serialization mirrors for `interchain-token-service-std`
//! and `its-borsh-translator` types.
//! These mirrors are necessary because Anchor's Borsh serialization
//! is not compatible with the Borsh serialization used in the cosmwasm
//! environment. Anchor currently uses borsh v0.10.4 while cosmwasm
//! requires 1.x.

use anchor_lang::prelude::*;

use crate::ItsError;

// Borsh-serializable mirror of interchain_token_service_std::InterchainTransfer
#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct InterchainTransfer {
    pub token_id: [u8; 32],
    pub source_address: Vec<u8>,
    pub destination_address: Vec<u8>,
    // Our program will convert this to u64, error
    // if too large
    // NOTE: this could be handled by the ITS cosmwasm contract
    // for more compact representation
    pub amount: [u8; 32], // Uint256 as 32-byte little-endian
    pub data: Option<Vec<u8>>,
}

/// Borsh-serializable mirror of interchain_token_service_std::DeployInterchainToken
#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct DeployInterchainToken {
    pub token_id: [u8; 32],
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
    pub minter: Option<Vec<u8>>,
}

/// Borsh-serializable mirror of interchain_token_service_std::LinkToken
#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct LinkToken {
    pub token_id: [u8; 32],
    pub token_manager_type: [u8; 32], // Uint256 as 32-byte little-endian
    pub source_token_address: Vec<u8>,
    pub destination_token_address: Vec<u8>,
    pub params: Option<Vec<u8>>,
}

/// Borsh-serializable mirror of interchain_token_service_std::RegisterTokenMetadata
#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct RegisterTokenMetadata {
    pub decimals: u8,
    pub token_address: Vec<u8>,
}

/// Borsh-serializable mirror of interchain_token_service_std::Message
/// Note: Borsh enums automatically serialize with a discriminant byte
#[derive(AnchorSerialize, AnchorDeserialize)]
pub enum Message {
    InterchainTransfer(InterchainTransfer),
    DeployInterchainToken(DeployInterchainToken),
    LinkToken(LinkToken),
}

/// Borsh-serializable mirror of interchain_token_service_std::HubMessage
#[derive(AnchorSerialize, AnchorDeserialize)]
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

//
// Utils
//

/// Convert a 32-byte little-endian array to u64, returning error if it overflows
pub fn u64_from_le_bytes_32(bytes: [u8; 32]) -> Result<u64> {
    // Check that upper 24 bytes are zero (value fits in u64)
    if bytes[8..].iter().any(|&b| b != 0) {
        return err!(ItsError::ArithmeticOverflow);
    }
    Ok(u64::from_le_bytes(bytes[..8].try_into().unwrap()))
}

/// Convert a 32-byte little-endian array to u8, returning error if it overflows
pub fn u8_from_le_bytes_32(bytes: [u8; 32]) -> Result<u8> {
    // Check that upper 31 bytes are zero (value fits in u8)
    if bytes[1..].iter().any(|&b| b != 0) {
        return err!(ItsError::ArithmeticOverflow);
    }
    Ok(bytes[0])
}

/// Convert a u64 to a 32-byte little-endian array
pub fn u64_to_le_bytes_32(value: u64) -> [u8; 32] {
    let mut bytes = [0u8; 32];
    bytes[..8].copy_from_slice(&value.to_le_bytes());
    bytes
}
