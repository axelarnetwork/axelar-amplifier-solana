#![deny(missing_docs)]

use anchor_lang::prelude::*;
use program_utils::pda::init_pda_raw_bytes;
use solana_program::pubkey::Pubkey;
use solana_program::entrypoint::ProgramResult;

use crate::find_transaction_pda;

#[derive(Debug, Eq, PartialEq, Clone, AnchorDeserialize, AnchorSerialize)]
/// A single piece of data to be passed by the relayer. Each of these can be converted to Vec<u8>.
pub enum RelayerData {
	/// Some raw bytes.
	Bytes(Vec<u8>),
	/// The message.
	Message,
	/// The payload, length prefixed.
	Payload,
	/// The payload, length ommitted.
	PayloadRaw,
	/// The command id. Can also be abtained by using the `Message`, but it is added as an option for convenience.
	CommandId,
}
#[derive(Debug, Eq, PartialEq, Clone, AnchorDeserialize, AnchorSerialize)]
/// This can be used to specify an account that the relayer will pass to the executable. This can be converted to an `AccountMeta` by the relayer.
pub enum RelayerAccount {
	/// This variant specifies a specific account. This account cannot be a signer (see `Payer` below).
	Account{
		/// The pubkey of the account.
		pubkey: Pubkey,
		/// Whether or not this account is writable.
		is_writable: bool,
	},
	/// An account that has the payload as its data. This account if and only if it is requested by the executable. This should only be specified once per instruction.
	MessagePayload,
	/// A signer account that has the amount of lamports specified. These lamports will be subtracted from the gas for the execution of the program. 
	/// This can be specified multiple times per instruction, and multiple payer accounts, funded differently will be provided. (Do we want this?)
	Payer(u64)
}
#[derive(Debug, Eq, PartialEq, Clone, AnchorDeserialize, AnchorSerialize)]
/// A relayer instruction, that the relayer can convert to an `Instruction`.
pub struct RelayerInstruction {
	/// The program_id. Note that this means that an executable can request the entrypoint be a different program (which would have to call the executable to validate the message).
	pub program_id: Pubkey,
	/// The instruction accounts. These need to be oredered properly.
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

impl RelayerTransaction {
	/// Helper function that serializes the enum. There must be a better way of doing this that escapes me, but variable length PDAs seem difficult with anchor. 
    ///
    /// # Arguments
    ///
    /// * `program_id` - The program id that is initializing their `transactionn_pda`.
    /// * `system_account` - The system account.
    /// * `payer` - The payer account, which needs to have enough lamports to pay for the initialization.
    /// * `into` - The `transaction_pda` for this `program_id`, which have the right key for the given `program_id`.
    ///
    /// # Returns
    /// 
    /// Returns the `ProgramResult` for the initialization.
	pub fn init<'a>(
        &self,
        program_id: &Pubkey,
        system_account: &AccountInfo<'a>,
        payer: &AccountInfo<'a>,
        into: &AccountInfo<'a>,
    ) -> ProgramResult {
		let mut serialized_data = Vec::with_capacity(256);
		self.serialize(&mut serialized_data)?;
		
		let (expected_pda, bump) = find_transaction_pda(program_id);
		if &expected_pda != into.key {
			return Err(ProgramError::InvalidAccountOwner);
		}
		
		let signer_seeds = [
			crate::TRANSACTION_PDA_SEED,
			&[bump],
		];
        init_pda_raw_bytes(
            payer,
            into,
            program_id,
            system_account,
            &serialized_data,
            &signer_seeds,
        )?;

        Ok(())
	}
}