//! Events emitted by the Axelar Solana Gas service

use anchor_lang::prelude::{
    borsh, event, AnchorDeserialize, AnchorSerialize, Discriminator, Pubkey,
};

/// Event emitted by the Axelar Solana Gas service
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum GasServiceEvent {
    /// Event when SOL was used to pay for a contract call
    NativeGasPaidForContractCall(NativeGasPaidForContractCallEvent),
    /// Event when SOL was added to fund an already emitted contract call
    NativeGasAdded(NativeGasAddedEvent),
    /// Event when SOL was refunded
    NativeGasRefunded(NativeGasRefundedEvent),
    /// Event when an SPL token was used to pay for a contract call
    SplGasPaidForContractCall(SplGasPaidForContractCallEvent),
    /// Event when an SPL token was added to fund an already emitted contract call
    SplGasAdded(SplGasAddedEvent),
    /// Event when an SPL token was refunded
    SplGasRefunded(SplGasRefundedEvent),
}

/// Represents the event emitted when native gas is paid for a contract call.
#[event]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct NativeGasPaidForContractCallEvent {
    /// The Gas service config PDA
    pub config_pda: Pubkey,
    /// Destination chain on the Axelar network
    pub destination_chain: String,
    /// Destination address on the Axelar network
    pub destination_address: String,
    /// The payload hash for the event we're paying for
    pub payload_hash: [u8; 32],
    /// The refund address
    pub refund_address: Pubkey,
    /// The amount of SOL to send
    pub gas_fee_amount: u64,
}

/// Represents the event emitted when native gas is added.
#[event]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct NativeGasAddedEvent {
    /// The Gas service config PDA
    pub config_pda: Pubkey,
    /// Solana transaction signature
    pub tx_hash: [u8; 64],
    /// index of the log
    pub log_index: u64,
    /// The refund address
    pub refund_address: Pubkey,
    /// amount of SOL
    pub gas_fee_amount: u64,
}

/// Represents the event emitted when native gas is refunded.
#[event]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct NativeGasRefundedEvent {
    /// Solana transaction signature
    pub tx_hash: [u8; 64],
    /// The Gas service config PDA
    pub config_pda: Pubkey,
    /// The log index
    pub log_index: u64,
    /// The receiver of the refund
    pub receiver: Pubkey,
    /// amount of SOL
    pub fees: u64,
}

/// Represents the event emitted when native gas is paid for a contract call.
#[event]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SplGasPaidForContractCallEvent {
    /// The Gas service config PDA
    pub config_pda: Pubkey,
    /// The Gas service config associated token account PDA
    pub config_pda_ata: Pubkey,
    /// Mint of the token
    pub mint: Pubkey,
    /// The token program id
    pub token_program_id: Pubkey,
    /// Destination chain on the Axelar network
    pub destination_chain: String,
    /// Destination address on the Axelar network
    pub destination_address: String,
    /// The payload hash for the event we're paying for
    pub payload_hash: [u8; 32],
    /// The refund address
    pub refund_address: Pubkey,
    /// The amount of SOL to send
    pub gas_fee_amount: u64,
}

/// Represents the event emitted when native gas is added.
#[event]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SplGasAddedEvent {
    /// The Gas service config PDA
    pub config_pda: Pubkey,
    /// The Gas service config associated token account PDA
    pub config_pda_ata: Pubkey,
    /// Mint of the token
    pub mint: Pubkey,
    /// The token program id
    pub token_program_id: Pubkey,
    /// Solana transaction signature
    pub tx_hash: [u8; 64],
    /// index of the log
    pub log_index: u64,
    /// The refund address
    pub refund_address: Pubkey,
    /// amount of SOL
    pub gas_fee_amount: u64,
}

/// Represents the event emitted when native gas is refunded.
#[event]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SplGasRefundedEvent {
    /// The Gas service config associated token account PDA
    pub config_pda_ata: Pubkey,
    /// Mint of the token
    pub mint: Pubkey,
    /// The token program id
    pub token_program_id: Pubkey,
    /// Solana transaction signature
    pub tx_hash: [u8; 64],
    /// The Gas service config PDA
    pub config_pda: Pubkey,
    /// The log index
    pub log_index: u64,
    /// The receiver of the refund
    pub receiver: Pubkey,
    /// amount of SOL
    pub fees: u64,
}
