//! Events emitted by the Axelar Solana Gas service

use anchor_lang::prelude::{
    borsh, event, AnchorDeserialize, AnchorSerialize, Discriminator, Pubkey,
};

use event_utils::{read_array, read_string, read_u64, EventParseError};

/// Even emitted by the Axelar Solana Gas service
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
    /// Extra parameters to be passed
    pub params: Vec<u8>,
    /// The amount of SOL to send
    pub gas_fee_amount: u64,
}

impl NativeGasPaidForContractCallEvent {
    /// Construct a new event from byte slices
    ///
    /// # Errors
    /// - if the data could not be parsed into an event
    pub fn new<I: Iterator<Item = Vec<u8>>>(mut data: I) -> Result<Self, EventParseError> {
        let treasury_data = data
            .next()
            .ok_or(EventParseError::MissingData("treasury"))?;
        let treasury = Pubkey::new_from_array(read_array::<32>("treasury", &treasury_data)?);

        let destination_chain_data = data
            .next()
            .ok_or(EventParseError::MissingData("destination_chain"))?;
        let destination_chain = read_string("destination_chain", destination_chain_data)?;

        let destination_address_data = data
            .next()
            .ok_or(EventParseError::MissingData("destination_address"))?;
        let destination_address = read_string("destination_address", destination_address_data)?;

        let payload_hash_data = data
            .next()
            .ok_or(EventParseError::MissingData("payload_hash"))?;
        let payload_hash = read_array::<32>("payload_hash", &payload_hash_data)?;

        let refund_address_data = data
            .next()
            .ok_or(EventParseError::MissingData("refund_address"))?;
        let refund_address =
            Pubkey::new_from_array(read_array::<32>("refund_address", &refund_address_data)?);

        let params = data.next().ok_or(EventParseError::MissingData("params"))?;

        let gas_fee_amount_data = data
            .next()
            .ok_or(EventParseError::MissingData("gas_fee_amount"))?;
        let gas_fee_amount = read_u64("gas_fee_amount", &gas_fee_amount_data)?;

        Ok(Self {
            treasury,
            destination_chain,
            destination_address,
            payload_hash,
            refund_address,
            params,
            gas_fee_amount,
        })
    }
}

/// Represents the event emitted when native gas is added.
#[event]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct NativeGasAddedEvent {
    /// The Gas service treasury PDA
    pub treasury: Pubkey,
    /// Solana transaction signature
    pub tx_hash: [u8; 64],
    /// index of the log
    pub log_index: u64,
    /// The refund address
    pub refund_address: Pubkey,
    /// amount of SOL
    pub gas_fee_amount: u64,
}

impl NativeGasAddedEvent {
    /// Construct a new event from byte slices
    ///
    /// # Errors
    /// - if the data could not be parsed into an event
    pub fn new<I: Iterator<Item = Vec<u8>>>(mut data: I) -> Result<Self, EventParseError> {
        let treasury_data = data
            .next()
            .ok_or(EventParseError::MissingData("treasury"))?;
        let treasury = Pubkey::new_from_array(read_array::<32>("treasury", &treasury_data)?);

        let tx_hash_data = data.next().ok_or(EventParseError::MissingData("tx_hash"))?;
        let tx_hash = read_array::<64>("tx_hash", &tx_hash_data)?;

        let log_index_data = data
            .next()
            .ok_or(EventParseError::MissingData("log_index"))?;
        let log_index = read_u64("log_index", &log_index_data)?;

        let refund_address_data = data
            .next()
            .ok_or(EventParseError::MissingData("refund_address"))?;
        let refund_address =
            Pubkey::new_from_array(read_array::<32>("refund_address", &refund_address_data)?);

        let gas_fee_amount_data = data
            .next()
            .ok_or(EventParseError::MissingData("gas_fee_amount"))?;
        let gas_fee_amount = read_u64("gas_fee_amount", &gas_fee_amount_data)?;

        Ok(Self {
            treasury,
            tx_hash,
            log_index,
            refund_address,
            gas_fee_amount,
        })
    }
}

/// Represents the event emitted when native gas is refunded.
#[event]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct NativeGasRefundedEvent {
    /// Solana transaction signature
    pub tx_hash: [u8; 64],
    /// The Gas service treasury PDA
    pub treasury: Pubkey,
    /// The log index
    pub log_index: u64,
    /// The receiver of the refund
    pub receiver: Pubkey,
    /// amount of SOL
    pub fees: u64,
}

impl NativeGasRefundedEvent {
    /// Construct a new event from byte slices
    ///
    /// # Errors
    /// - if the data could not be parsed into an event
    pub fn new<I: Iterator<Item = Vec<u8>>>(mut data: I) -> Result<Self, EventParseError> {
        let tx_hash_data = data.next().ok_or(EventParseError::MissingData("tx_hash"))?;
        let tx_hash = read_array::<64>("tx_hash", &tx_hash_data)?;

        let treasury_data = data
            .next()
            .ok_or(EventParseError::MissingData("treasury"))?;
        let treasury = Pubkey::new_from_array(read_array::<32>("treasury", &treasury_data)?);

        let log_index_data = data
            .next()
            .ok_or(EventParseError::MissingData("log_index"))?;
        let log_index = read_u64("log_index", &log_index_data)?;

        let receiver_data = data
            .next()
            .ok_or(EventParseError::MissingData("receiver"))?;
        let receiver = Pubkey::new_from_array(read_array::<32>("receiver", &receiver_data)?);

        let fees_data = data.next().ok_or(EventParseError::MissingData("fees"))?;
        let fees = read_u64("fees", &fees_data)?;

        Ok(Self {
            tx_hash,
            treasury,
            log_index,
            receiver,
            fees,
        })
    }
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
    /// Extra parameters to be passed
    pub params: Vec<u8>,
    /// The amount of SOL to send
    pub gas_fee_amount: u64,
}

impl SplGasPaidForContractCallEvent {
    /// Construct a new event from byte slices
    ///
    /// # Errors
    /// - if the data could not be parsed into an event
    pub fn new<I: Iterator<Item = Vec<u8>>>(mut data: I) -> Result<Self, EventParseError> {
        let treasury_data = data
            .next()
            .ok_or(EventParseError::MissingData("treasury"))?;
        let treasury = Pubkey::new_from_array(read_array::<32>("treasury", &treasury_data)?);

        let treasury_token_account = data
            .next()
            .ok_or(EventParseError::MissingData("treasury_token_account"))?;
        let treasury_token_account = Pubkey::new_from_array(read_array::<32>(
            "treasury_token_account",
            &treasury_token_account,
        )?);

        let mint = data.next().ok_or(EventParseError::MissingData("mint"))?;
        let mint = Pubkey::new_from_array(read_array::<32>("mint", &mint)?);

        let token_program_id = data
            .next()
            .ok_or(EventParseError::MissingData("token_program_id"))?;
        let token_program_id =
            Pubkey::new_from_array(read_array::<32>("token_program_id", &token_program_id)?);

        let destination_chain_data = data
            .next()
            .ok_or(EventParseError::MissingData("destination_chain"))?;
        let destination_chain = read_string("destination_chain", destination_chain_data)?;

        let destination_address_data = data
            .next()
            .ok_or(EventParseError::MissingData("destination_address"))?;
        let destination_address = read_string("destination_address", destination_address_data)?;

        let payload_hash_data = data
            .next()
            .ok_or(EventParseError::MissingData("payload_hash"))?;
        let payload_hash = read_array::<32>("payload_hash", &payload_hash_data)?;

        let refund_address_data = data
            .next()
            .ok_or(EventParseError::MissingData("refund_address"))?;
        let refund_address =
            Pubkey::new_from_array(read_array::<32>("refund_address", &refund_address_data)?);

        let params = data.next().ok_or(EventParseError::MissingData("params"))?;

        let gas_fee_amount_data = data
            .next()
            .ok_or(EventParseError::MissingData("gas_fee_amount"))?;
        let gas_fee_amount = read_u64("gas_fee_amount", &gas_fee_amount_data)?;

        Ok(Self {
            treasury,
            treasury_token_account,
            mint,
            token_program_id,
            destination_chain,
            destination_address,
            payload_hash,
            refund_address,
            params,
            gas_fee_amount,
        })
    }
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
    /// index of the log
    pub log_index: u64,
    /// The refund address
    pub refund_address: Pubkey,
    /// amount of SOL
    pub gas_fee_amount: u64,
}

impl SplGasAddedEvent {
    /// Construct a new event from byte slices
    ///
    /// # Errors
    /// - if the data could not be parsed into an event
    pub fn new<I: Iterator<Item = Vec<u8>>>(mut data: I) -> Result<Self, EventParseError> {
        let treasury_data = data
            .next()
            .ok_or(EventParseError::MissingData("treasury"))?;
        let treasury = Pubkey::new_from_array(read_array::<32>("treasury", &treasury_data)?);

        let treasury_token_account = data
            .next()
            .ok_or(EventParseError::MissingData("treasury_token_account"))?;
        let treasury_token_account = Pubkey::new_from_array(read_array::<32>(
            "treasury_token_account",
            &treasury_token_account,
        )?);

        let mint = data.next().ok_or(EventParseError::MissingData("mint"))?;
        let mint = Pubkey::new_from_array(read_array::<32>("mint", &mint)?);

        let token_program_id = data
            .next()
            .ok_or(EventParseError::MissingData("token_program_id"))?;
        let token_program_id =
            Pubkey::new_from_array(read_array::<32>("token_program_id", &token_program_id)?);

        let tx_hash_data = data.next().ok_or(EventParseError::MissingData("tx_hash"))?;
        let tx_hash = read_array::<64>("tx_hash", &tx_hash_data)?;

        let log_index_data = data
            .next()
            .ok_or(EventParseError::MissingData("log_index"))?;
        let log_index = read_u64("log_index", &log_index_data)?;

        let refund_address_data = data
            .next()
            .ok_or(EventParseError::MissingData("refund_address"))?;
        let refund_address =
            Pubkey::new_from_array(read_array::<32>("refund_address", &refund_address_data)?);

        let gas_fee_amount_data = data
            .next()
            .ok_or(EventParseError::MissingData("gas_fee_amount"))?;
        let gas_fee_amount = read_u64("gas_fee_amount", &gas_fee_amount_data)?;

        Ok(Self {
            treasury,
            treasury_token_account,
            mint,
            token_program_id,
            tx_hash,
            log_index,
            refund_address,
            gas_fee_amount,
        })
    }
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
    /// The log index
    pub log_index: u64,
    /// The receiver of the refund
    pub receiver: Pubkey,
    /// amount of SOL
    pub fees: u64,
}

impl SplGasRefundedEvent {
    /// Construct a new event from byte slices
    ///
    /// # Errors
    /// - if the data could not be parsed into an event
    pub fn new<I: Iterator<Item = Vec<u8>>>(mut data: I) -> Result<Self, EventParseError> {
        let tx_hash_data = data.next().ok_or(EventParseError::MissingData("tx_hash"))?;
        let tx_hash = read_array::<64>("tx_hash", &tx_hash_data)?;

        let treasury_data = data
            .next()
            .ok_or(EventParseError::MissingData("treasury"))?;
        let treasury = Pubkey::new_from_array(read_array::<32>("treasury", &treasury_data)?);

        let treasury_token_account = data
            .next()
            .ok_or(EventParseError::MissingData("treasury_token_account"))?;
        let treasury_token_account = Pubkey::new_from_array(read_array::<32>(
            "treasury_token_account",
            &treasury_token_account,
        )?);

        let mint = data.next().ok_or(EventParseError::MissingData("mint"))?;
        let mint = Pubkey::new_from_array(read_array::<32>("mint", &mint)?);

        let token_program_id = data
            .next()
            .ok_or(EventParseError::MissingData("token_program_id"))?;
        let token_program_id =
            Pubkey::new_from_array(read_array::<32>("token_program_id", &token_program_id)?);

        let log_index_data = data
            .next()
            .ok_or(EventParseError::MissingData("log_index"))?;
        let log_index = read_u64("log_index", &log_index_data)?;

        let receiver_data = data
            .next()
            .ok_or(EventParseError::MissingData("receiver"))?;
        let receiver = Pubkey::new_from_array(read_array::<32>("receiver", &receiver_data)?);

        let fees_data = data.next().ok_or(EventParseError::MissingData("fees"))?;
        let fees = read_u64("fees", &fees_data)?;

        Ok(Self {
            treasury_token_account,
            mint,
            token_program_id,
            tx_hash,
            treasury,
            log_index,
            receiver,
            fees,
        })
    }
}
