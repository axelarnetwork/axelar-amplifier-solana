#![allow(clippy::too_many_arguments)]

use anchor_lang::prelude::thiserror;
use anchor_lang::{AnchorDeserialize, Key};

use axelar_solana_encoding::{hasher::SolanaSyscallHasher, rs_merkle::MerkleTree};

use libsecp256k1::SecretKey;
use mollusk_svm::result::InstructionResult;
use mollusk_svm::result::ProgramResult;
use relayer_discovery::structs::RelayerTransaction;
use relayer_discovery::{find_transaction_pda, ConvertError, RelayerDiscovery};
use solana_axelar_gateway::{Message, MessageLeaf, VerifierSetLeaf};
use solana_axelar_gateway_test_fixtures::{
    approve_message_helper, create_verifier_info, initialize_gateway,
    initialize_payload_verification_session_with_root, setup_test_with_real_signers,
    verify_signature_helper, TestSetup,
};
use solana_sdk::pubkey::ParsePubkeyError;
use solana_sdk::{
    account::Account, instruction::Instruction, pubkey::Pubkey,
    system_program::ID as SYSTEM_PROGRAM_ID,
};
use std::str::FromStr;

/// A complete setup for testing executables
pub struct RelayerDiscoveryTestFixture {
    /// This has the mollusk aslongside a lot of information about the gateway
    pub setup: TestSetup,
    /// The rest of the information is required to approve massages to the gateway
    verifier_leaves: Vec<VerifierSetLeaf>,
    verifier_merkle_tree: MerkleTree<SolanaSyscallHasher>,
    secret_key_1: SecretKey,
    secret_key_2: SecretKey,
    init_result: InstructionResult,
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
        let (setup, verifier_leaves, verifier_merkle_tree, secret_key_1, secret_key_2) =
            setup_test_with_real_signers();

        // Step 2: Initialize gateway
        let init_result = initialize_gateway(&setup);

        RelayerDiscoveryTestFixture {
            setup,
            verifier_leaves,
            verifier_merkle_tree,
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

        let message_leaves: Vec<MessageLeaf> = messages
            .iter()
            .enumerate()
            .map(|(i, msg)| MessageLeaf {
                message: msg.clone(),
                position: u16::try_from(i).unwrap(),
                set_size: u16::try_from(messages.len()).unwrap(),
                domain_separator: self.setup.domain_separator,
            })
            .collect();

        let message_leaf_hashes: Vec<[u8; 32]> =
            message_leaves.iter().map(MessageLeaf::hash).collect();

        let message_merkle_tree =
            MerkleTree::<SolanaSyscallHasher>::from_leaves(&message_leaf_hashes);
        let payload_merkle_root = message_merkle_tree.root().unwrap();

        // Step 4: Initialize payload verification session
        let (session_result, verification_session_pda) =
            initialize_payload_verification_session_with_root(
                &self.setup,
                &self.init_result,
                payload_merkle_root,
            );

        let gateway_root_account = self
            .init_result
            .get_account(&self.setup.gateway_root_pda)
            .unwrap();

        let verifier_set_tracker_account = self
            .init_result
            .get_account(&self.setup.verifier_set_tracker_pda)
            .unwrap();

        let verification_session_account = session_result
            .get_account(&verification_session_pda)
            .unwrap();

        // Step 5: Sign the payload with both signers, verify both signatures on the gateway
        let verifier_info_1 = create_verifier_info(
            &self.secret_key_1,
            payload_merkle_root,
            &self.verifier_leaves[0],
            0, // Position 0
            &self.verifier_merkle_tree,
        );

        let verify_result_1 = verify_signature_helper(
            &self.setup,
            payload_merkle_root,
            verifier_info_1,
            verification_session_pda,
            gateway_root_account.clone(),
            verification_session_account.clone(),
            self.setup.verifier_set_tracker_pda,
            verifier_set_tracker_account.clone(),
        );

        let updated_verification_account_after_first = verify_result_1
            .get_account(&verification_session_pda)
            .unwrap();

        let verifier_info_2 = create_verifier_info(
            &self.secret_key_2,
            payload_merkle_root,
            &self.verifier_leaves[1],
            1, // Position 1
            &self.verifier_merkle_tree,
        );

        let verify_result_2 = verify_signature_helper(
            &self.setup,
            payload_merkle_root,
            verifier_info_2,
            verification_session_pda,
            gateway_root_account.clone(),
            updated_verification_account_after_first.clone(),
            self.setup.verifier_set_tracker_pda,
            verifier_set_tracker_account.clone(),
        );

        // Step 6: Approve the message
        let (approve_result, _) = approve_message_helper(
            &self.setup,
            message_merkle_tree,
            message_leaves,
            &messages,
            payload_merkle_root,
            verification_session_pda,
            verify_result_2,
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
                    let instruction = relayer_discovery.convert_instruction(relayer_instruction);
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
                                _ => {
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
                            _ => {
                                return Err(error.into());
                            }
                        },
                    }
                }
                RelayerTransaction::Final(ref relayer_instructions) => {
                    let instructions: Result<Vec<Instruction>, ConvertError> = relayer_instructions
                        .iter()
                        .map(|relayer_instruction| {
                            relayer_discovery.convert_instruction(relayer_instruction)
                        })
                        .collect();
                    match instructions {
                        Ok(instructions) => {
                            // Add all accounts that are potentially empty.
                            instructions.iter().for_each(|instruction| {
                                instruction.accounts.iter().for_each(|account| {
                                    if accounts
                                        .iter()
                                        .find(|(existing, _)| existing == &account.pubkey)
                                        .is_none()
                                    {
                                        if !account.is_signer {
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
                                })
                            });
                            let result = self
                                .setup
                                .mollusk
                                .process_instruction_chain(&instructions, &accounts);
                            match result.program_result {
                                ProgramResult::Success => {
                                    return Ok(result);
                                }
                                _ => {
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
                            _ => {
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
