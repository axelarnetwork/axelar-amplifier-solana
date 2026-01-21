#![allow(clippy::too_many_arguments)]
#![allow(clippy::indexing_slicing)]
#![allow(clippy::too_many_lines)]

use anchor_lang::prelude::thiserror;
use anchor_lang::{AnchorDeserialize, Key};

use solana_axelar_std::{Message, Messages, Payload, PayloadType};

use libsecp256k1::SecretKey;
use mollusk_svm::result::InstructionResult;
use mollusk_svm::result::ProgramResult;
use relayer_discovery::structs::RelayerTransaction;
use relayer_discovery::{find_transaction_pda, ConvertError, RelayerDiscovery};
use solana_axelar_gateway_test_fixtures::{
    approve_message_helper, create_merklized_messages_from_std, create_signing_verifier_set_leaves,
    initialize_gateway, initialize_payload_verification_session, setup_test_with_real_signers,
    verify_signature_helper, TestSetup,
};
use solana_sdk::pubkey::ParsePubkeyError;
use solana_sdk::{account::Account, instruction::Instruction, pubkey::Pubkey};
use solana_sdk_ids::system_program::ID as SYSTEM_PROGRAM_ID;
use std::str::FromStr;

/// A complete setup for testing executables
pub struct RelayerDiscoveryTestFixture {
    /// This has the mollusk aslongside a lot of information about the gateway
    pub setup: TestSetup,
    /// The rest of the information is required to approve massages to the gateway
    pub secret_key_1: SecretKey,
    pub secret_key_2: SecretKey,
    pub init_result: InstructionResult,
}

#[derive(Debug, thiserror::Error)]
pub enum RelayerDiscoveryFixtureError {
    #[error("the pda that was expected to have the initial `RelayerTransaction` was not provided")]
    NoInitialRelayerTransaction,
    #[error("the destination address could not be parsed into a Solana address")]
    InvalidMessageDestinationAddress(ParsePubkeyError),
    #[error("desiralization error")]
    DesirializationError(std::io::Error),
    #[error("a discovery instruction failed when run")]
    DiscoveryInstructionFailed(Instruction, ProgramResult),
    #[error("relayer discovery failed to parse a transaction")]
    ConvertError(ConvertError),
    #[error("execution of final transaction failed")]
    ExecuteFailed(Vec<Instruction>, ProgramResult),
}

impl From<ParsePubkeyError> for RelayerDiscoveryFixtureError {
    fn from(value: ParsePubkeyError) -> Self {
        Self::InvalidMessageDestinationAddress(value)
    }
}

impl From<ConvertError> for RelayerDiscoveryFixtureError {
    fn from(value: ConvertError) -> Self {
        Self::ConvertError(value)
    }
}

impl From<std::io::Error> for RelayerDiscoveryFixtureError {
    fn from(value: std::io::Error) -> Self {
        Self::DesirializationError(value)
    }
}

impl RelayerDiscoveryTestFixture {
    /// Approve a certain message
    ///
    /// # Returns
    ///
    /// Returns the `RelayerDiscoveryTestFixture` generated.
    pub fn new() -> RelayerDiscoveryTestFixture {
        let (setup, secret_key_1, secret_key_2) = setup_test_with_real_signers();

        // Step 2: Initialize gateway
        let init_result = initialize_gateway(&setup);

        RelayerDiscoveryTestFixture {
            setup,
            secret_key_1,
            secret_key_2,
            init_result,
        }
    }

    /// Approve a certain message
    ///
    /// # Arguments
    ///
    /// * `message` - The message to be approved
    ///
    /// # Returns
    ///
    /// Returns the `InstructionResult` of the approval, which can be used to find the `incoming_message` Account.
    pub fn approve(&mut self, message: &Message) -> InstructionResult {
        let messages = vec![message.clone()];

        // Create payload merkle root using std crate approach
        let (_, payload_merkle_root) =
            create_merklized_messages_from_std(self.setup.domain_separator, &messages);

        let gateway_root_account = self
            .init_result
            .get_account(&self.setup.gateway_root_pda)
            .unwrap();

        let verifier_set_tracker_account = self
            .init_result
            .get_account(&self.setup.verifier_set_tracker_pda)
            .unwrap()
            .clone();

        let (session_result, verification_session_pda) = initialize_payload_verification_session(
            &self.setup,
            gateway_root_account.clone(),
            verifier_set_tracker_account.clone(),
            payload_merkle_root,
            PayloadType::ApproveMessages,
        );
        let verification_session_account = session_result
            .get_account(&verification_session_pda)
            .unwrap();

        // Step 5: Sign the payload with both signers, verify both signatures on the gateway
        let payload_to_be_signed = Payload::Messages(Messages(messages.clone()));
        let signing_verifier_set_leaves = create_signing_verifier_set_leaves(
            self.setup.domain_separator,
            &self.secret_key_1,
            &self.secret_key_2,
            payload_to_be_signed,
            self.setup.verifier_set.clone(),
        );

        let verifier_info_1 = signing_verifier_set_leaves[0].clone();

        let verify_result_1 = verify_signature_helper(
            &self.setup,
            payload_merkle_root,
            verifier_info_1,
            (
                verification_session_pda,
                verification_session_account.clone(),
            ),
            gateway_root_account.clone(),
            (
                self.setup.verifier_set_tracker_pda,
                verifier_set_tracker_account.clone(),
            ),
        );

        let updated_verification_account_after_first = verify_result_1
            .get_account(&verification_session_pda)
            .unwrap()
            .clone();

        let verifier_info_2 = signing_verifier_set_leaves[1].clone();

        let verify_result_2 = verify_signature_helper(
            &self.setup,
            payload_merkle_root,
            verifier_info_2,
            (
                verification_session_pda,
                updated_verification_account_after_first.clone(),
            ),
            gateway_root_account.clone(),
            (
                self.setup.verifier_set_tracker_pda,
                verifier_set_tracker_account.clone(),
            ),
        );

        // Step 6: Approve the message
        let final_gateway_account = verify_result_2
            .get_account(&self.setup.gateway_root_pda)
            .unwrap()
            .clone();
        let final_verification_session_account = verify_result_2
            .get_account(&verification_session_pda)
            .unwrap()
            .clone();

        let (approve_result, _) = approve_message_helper(
            &self.setup,
            &messages,
            (verification_session_pda, final_verification_session_account),
            final_gateway_account,
            0, // position
        );

        assert!(
            !approve_result.program_result.is_err(),
            "Message approval should succeed"
        );
        approve_result
    }

    /// Execute a certain message.
    /// Will add all the necesairy requested payers and a payload account if needed.
    ///
    /// # Arguments
    ///
    /// * `message` - The message to be executed
    /// * `payload` - The incoming payload (its hash has to match `message.payload_hash`)
    /// * `accounts` - The accounts needed for execution, including the `incoming_message` as well as all the accouns that the executable should have initialized by this point.
    ///
    /// # Returns
    ///
    /// Returns `Ok(InstructionResult)` if the execution was successful, if any error was encountered then `Err(RelayerDiscoveryFixtureError)` is returned.
    pub fn execute(
        &mut self,
        message: &Message,
        payload: Vec<u8>,
        mut accounts: Vec<(Pubkey, Account)>,
    ) -> Result<InstructionResult, RelayerDiscoveryFixtureError> {
        let mut relayer_discovery = RelayerDiscovery {
            message: message.clone(),
            payload,
            payload_pda: None,
            payers: vec![],
        };

        let (relayer_transaction_pda, _) =
            find_transaction_pda(&Pubkey::from_str(&message.destination_address)?);
        let (_, relayer_transaction_account) = accounts
            .iter()
            .find(|(pubkey, _)| pubkey == &relayer_transaction_pda)
            .ok_or(RelayerDiscoveryFixtureError::NoInitialRelayerTransaction)?;
        let mut buffer = &relayer_transaction_account.data.clone();
        let mut relayer_transaction = RelayerTransaction::deserialize(&mut buffer.as_slice())?;
        loop {
            match relayer_transaction {
                RelayerTransaction::Discovery(ref relayer_instruction) => {
                    let instruction =
                        relayer_discovery.convert_instruction(relayer_instruction, &mut vec![]);
                    match instruction {
                        Ok(instruction) => {
                            let result = self
                                .setup
                                .mollusk
                                .process_instruction(&instruction, &accounts);
                            match result.program_result {
                                ProgramResult::Success => {
                                    buffer = &result.return_data;
                                    relayer_transaction =
                                        RelayerTransaction::deserialize(&mut buffer.as_slice())?;
                                }
                                ProgramResult::Failure(_) | ProgramResult::UnknownError(_) => {
                                    return Err(
                                        RelayerDiscoveryFixtureError::DiscoveryInstructionFailed(
                                            instruction,
                                            result.program_result,
                                        ),
                                    );
                                }
                            }
                        }
                        Err(error) => match error {
                            ConvertError::NeedMessagePayload => {
                                let account = (
                                    Pubkey::new_unique(),
                                    Account {
                                        lamports: 0,
                                        data: relayer_discovery.payload.clone(),
                                        owner: SYSTEM_PROGRAM_ID,
                                        executable: false,
                                        rent_epoch: 0,
                                    },
                                );
                                relayer_discovery.add_payload_pda(account.0);
                                accounts.push(account);
                            }
                            ConvertError::NeedPayer(amount) => {
                                let account = (
                                    Pubkey::new_unique(),
                                    Account {
                                        lamports: amount,
                                        data: vec![],
                                        owner: SYSTEM_PROGRAM_ID,
                                        executable: false,
                                        rent_epoch: 0,
                                    },
                                );
                                relayer_discovery.add_payer(account.0, amount);
                                accounts.push(account);
                            }
                            ConvertError::FailedMessageSerialization
                            | ConvertError::FailedPayloadSerialization => {
                                return Err(error.into());
                            }
                        },
                    }
                }
                RelayerTransaction::Final(ref relayer_instructions) => {
                    let mut used_payers = vec![];
                    let instructions: Result<Vec<Instruction>, ConvertError> = relayer_instructions
                        .iter()
                        .map(|relayer_instruction| {
                            relayer_discovery
                                .convert_instruction(relayer_instruction, &mut used_payers)
                        })
                        .collect();
                    match instructions {
                        Ok(instructions) => {
                            // Add all accounts that are potentially empty.
                            for instruction in instructions.iter() {
                                for account in instruction.accounts.iter() {
                                    if !accounts
                                        .iter()
                                        .any(|(existing, _)| existing == &account.pubkey)
                                        && !account.is_signer
                                    {
                                        accounts.push((
                                            account.pubkey,
                                            Account {
                                                lamports: 0,
                                                data: vec![],
                                                owner: SYSTEM_PROGRAM_ID,
                                                executable: false,
                                                rent_epoch: 0,
                                            },
                                        ));
                                    }
                                }
                            }
                            let result = self
                                .setup
                                .mollusk
                                .process_instruction_chain(&instructions, &accounts);
                            match result.program_result {
                                ProgramResult::Success => {
                                    return Ok(result);
                                }
                                ProgramResult::Failure(_) | ProgramResult::UnknownError(_) => {
                                    return Err(RelayerDiscoveryFixtureError::ExecuteFailed(
                                        instructions,
                                        result.program_result,
                                    ));
                                }
                            }
                        }
                        Err(error) => match error {
                            ConvertError::NeedMessagePayload => {
                                let account = (
                                    Pubkey::new_unique(),
                                    Account {
                                        lamports: 0,
                                        data: relayer_discovery.payload.clone(),
                                        owner: self.setup.payer.key(),
                                        executable: false,
                                        rent_epoch: 0,
                                    },
                                );
                                relayer_discovery.add_payload_pda(account.0);
                                accounts.push(account);
                            }
                            ConvertError::NeedPayer(amount) => {
                                let account = (
                                    Pubkey::new_unique(),
                                    Account {
                                        lamports: amount,
                                        data: vec![],
                                        owner: SYSTEM_PROGRAM_ID,
                                        executable: false,
                                        rent_epoch: 0,
                                    },
                                );
                                relayer_discovery.add_payer(account.0, amount);
                                accounts.push(account);
                            }
                            ConvertError::FailedMessageSerialization
                            | ConvertError::FailedPayloadSerialization => {
                                return Err(error.into());
                            }
                        },
                    }
                }
            }
        }
    }

    /// Approve and execute a certain message. Basically a chained `approve` and `execute`.
    /// Will add all the necesairy requested payers and a payload account if needed.
    ///
    /// # Arguments
    ///
    /// * `message` - The message to be executed
    /// * `payload` - The incoming payload (its hash has to match `message.payload_hash`)
    /// * `accounts` - The accounts needed for execution, which should be the accouns that the executable should have initialized by this point.
    ///
    /// # Returns
    ///
    /// Returns `Ok(InstructionResult)` if the execution was successful, if any error was encountered then `Err(RelayerDiscoveryFixtureError)` is returned.
    pub fn approve_and_execute(
        &mut self,
        message: &Message,
        payload: Vec<u8>,
        mut accounts: Vec<(Pubkey, Account)>,
    ) -> Result<InstructionResult, RelayerDiscoveryFixtureError> {
        let mut approval_result = self.approve(message);
        accounts.append(&mut approval_result.resulting_accounts);
        self.execute(message, payload, accounts)
    }
}

impl Default for RelayerDiscoveryTestFixture {
    fn default() -> Self {
        RelayerDiscoveryTestFixture::new()
    }
}
