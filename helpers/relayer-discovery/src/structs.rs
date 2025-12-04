#![deny(missing_docs)]

use anchor_lang::prelude::*;
use solana_program::pubkey::Pubkey;

#[derive(Debug, Eq, PartialEq, Clone, AnchorDeserialize, AnchorSerialize)]
/// A single piece of data to be passed by the relayer. Each of these can be converted to Vec<u8>.
pub enum RelayerData {
    /// Some raw bytes.
    Bytes(Vec<u8>),
    /// The message.
    Message,
    /// The payload, length prefixed.
    Payload,
    /// The payload, length omitted.
    PayloadRaw,
    /// The command id. Can also be abtained by using the `Message`, but it is added as an option for convenience.
    CommandId,
}

impl RelayerData {
    /// Serialize some input params.
    pub fn from_serializable<T: AnchorSerialize>(data: T) -> Result<RelayerData> {
        let mut result = Vec::with_capacity(256);
        data.serialize(&mut result)?;
        Ok(RelayerData::Bytes(result))
    }
}
#[derive(Debug, Eq, PartialEq, Clone, AnchorDeserialize, AnchorSerialize)]
/// This can be used to specify an account that the relayer will pass to the executable. This can be converted to an `AccountMeta` by the relayer.
pub enum RelayerAccount {
    /// This variant specifies a specific account. This account cannot be a signer (see `Payer` below).
    Account {
        /// The pubkey of the account.
        pubkey: Pubkey,
        /// Whether or not this account is writable.
        is_writable: bool,
    },
    /// An account that has the payload as its data. This account if and only if it is requested by the executable. This should only be specified once per instruction.
    MessagePayload,
    /// A signer account that has the amount of lamports specified. These lamports will be subtracted from the gas for the execution of the program.
    /// This can be specified multiple times per instruction, and multiple payer accounts, funded differently will be provided. (Do we want this?)
    Payer(u64),
}
#[derive(Debug, Eq, PartialEq, Clone, AnchorDeserialize, AnchorSerialize)]
/// A relayer instruction, that the relayer can convert to an `Instruction`.
pub struct RelayerInstruction {
    /// The program_id. Note that this means that an executable can request the entrypoint be a different program (which would have to call the executable to validate the message).
    pub program_id: Pubkey,
    /// The instruction accounts. These need to be ordered properly.
    pub accounts: Vec<RelayerAccount>,
    /// The instruction data. These will be concatenated.
    pub data: Vec<RelayerData>,
}

#[derive(Debug, Eq, PartialEq, Clone, AnchorDeserialize, AnchorSerialize)]
/// A relayer transaction, that the relayer can convert to regular transaction.
pub enum RelayerTransaction {
    /// This series of instructions should be executed.
    Final(Vec<RelayerInstruction>),
    /// This instruction should be simulated to eventually get a `Final` transaction.
    Discovery(RelayerInstruction),
}
