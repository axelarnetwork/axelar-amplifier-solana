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
    /// The Gas service treasury PDA
    pub treasury: Pubkey,
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
    /// The Gas service treasury PDA
    pub treasury: Pubkey,
    /// Solana transaction signature
    pub tx_hash: [u8; 64],
    /// Index of the CallContract instruction
    pub ix_index: u8,
    /// Index of the CPI event inside inner instructions
    pub event_ix_index: u8,
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
    /// The Gas service treasury PDA
    pub treasury: Pubkey,
    /// Index of the CallContract instruction
    pub ix_index: u8,
    /// Index of the CPI event inside inner instructions
    pub event_ix_index: u8,
    /// The receiver of the refund
    pub receiver: Pubkey,
    /// amount of SOL
    pub fees: u64,
}

/// Represents the event emitted when native gas is paid for a contract call.
#[event]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SplGasPaidForContractCallEvent {
    /// The Gas service treasury PDA
    pub treasury: Pubkey,
    /// The Gas service treasury token account PDA
    pub treasury_token_account: Pubkey,
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
    /// The Gas service treasury PDA
    pub treasury: Pubkey,
    /// The Gas service treasury token account PDA
    pub treasury_token_account: Pubkey,
    /// Mint of the token
    pub mint: Pubkey,
    /// The token program id
    pub token_program_id: Pubkey,
    /// Solana transaction signature
    pub tx_hash: [u8; 64],
    /// Index of the CallContract instruction
    pub ix_index: u8,
    /// Index of the CPI event inside inner instructions
    pub event_ix_index: u8,
    /// The refund address
    pub refund_address: Pubkey,
    /// amount of SOL
    pub gas_fee_amount: u64,
}

/// Represents the event emitted when native gas is refunded.
#[event]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SplGasRefundedEvent {
    /// The Gas service treasury token account PDA
    pub treasury_token_account: Pubkey,
    /// Mint of the token
    pub mint: Pubkey,
    /// The token program id
    pub token_program_id: Pubkey,
    /// Solana transaction signature
    pub tx_hash: [u8; 64],
    /// The Gas service treasury PDA
    pub treasury: Pubkey,
    /// Index of the CallContract instruction
    pub ix_index: u8,
    /// Index of the CPI event inside inner instructions
    pub event_ix_index: u8,
    /// The receiver of the refund
    pub receiver: Pubkey,
    /// amount of SOL
    pub fees: u64,
}
