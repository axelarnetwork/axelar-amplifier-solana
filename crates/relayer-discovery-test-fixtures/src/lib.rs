#![allow(clippy::too_many_arguments)]

use anchor_lang::{AnchorDeserialize, Key};
use anchor_lang::prelude::thiserror;

use axelar_solana_encoding::{hasher::SolanaSyscallHasher, rs_merkle::MerkleTree};

use axelar_solana_gateway_v2::{
    Message, MessageLeaf, VerifierSetLeaf,
};
use libsecp256k1::SecretKey;
use mollusk_svm::result::ProgramResult;
use mollusk_svm::result::InstructionResult;
use relayer_discovery::structs::RelayerTransaction;
use solana_sdk::pubkey::ParsePubkeyError;
use solana_sdk::{
    account::Account,
    instruction::Instruction,
    pubkey::Pubkey,
};
use axelar_solana_gateway_v2_test_fixtures::{
    TestSetup, approve_message_helper, create_verifier_info, initialize_gateway, initialize_payload_verification_session_with_root, setup_test_with_real_signers, verify_signature_helper
};
use relayer_discovery::{ConvertError, RelayerDiscovery, find_transaction_pda};
use std::str::FromStr;

pub struct RelayerDiscoveryTestFixture {
    pub setup: TestSetup,
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
    #[error("the pda that was expected to have the initial `RelayerTransaction` was not provided")]
    InvalidMessageDestinationAddress(ParsePubkeyError),
    #[error("the pda that was expected to have the initial `RelayerTransaction` was not provided")]
    DesirializationError(std::io::Error),
    #[error("the pda that was expected to have the initial `RelayerTransaction` was not provided")]
    DiscoveryInstructionFailed(Instruction, ProgramResult),
    #[error("the pda that was expected to have the initial `RelayerTransaction` was not provided")]
    ConvertError(ConvertError),
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
    pub fn new() -> RelayerDiscoveryTestFixture {
        let (setup, verifier_leaves, verifier_merkle_tree, secret_key_1, secret_key_2) =
        setup_test_with_real_signers();


        // Step 2: Initialize gateway
        let init_result = initialize_gateway(&setup);

        RelayerDiscoveryTestFixture { setup, verifier_leaves, verifier_merkle_tree, secret_key_1, secret_key_2, init_result }
    }

    pub fn approve(&mut self, message: &Message) -> Pubkey {
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

        let message_leaf_hashes: Vec<[u8; 32]> = message_leaves
            .iter()
            .map(axelar_solana_gateway_v2::MessageLeaf::hash)
            .collect();

        let message_merkle_tree = MerkleTree::<SolanaSyscallHasher>::from_leaves(&message_leaf_hashes);
        let payload_merkle_root = message_merkle_tree.root().unwrap();

        // Step 4: Initialize payload verification session
        let (session_result, verification_session_pda) =
            initialize_payload_verification_session_with_root(
                &self.setup,
                &self.init_result,
                payload_merkle_root,
            );

        let gateway_root_account = self.init_result.get_account(&self.setup.gateway_root_pda).unwrap();

        let verifier_set_tracker_account = self.init_result
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
        let (approve_result, incoming_message_pda) = approve_message_helper(
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
        incoming_message_pda
    }

    pub fn execute(&mut self, message: &Message, incoming_message_pda: Pubkey, payload: Vec<u8>, mut accounts: Vec<(Pubkey, Account)>) -> Result<(), RelayerDiscoveryFixtureError> {
        let mut relayer_discovery = RelayerDiscovery {
            message: message.clone(),
            message_pda: incoming_message_pda,
            payload,
            payload_pda: None,
            payers: vec![],
        };

        let (relayer_transaction_pda, _) = find_transaction_pda(
            &Pubkey::from_str(
                &message.destination_address
            )?
        );
        let (_, relayer_transaction_account) = accounts.iter().find(|(pubkey, _)| pubkey == &relayer_transaction_pda).ok_or(RelayerDiscoveryFixtureError::NoInitialRelayerTransaction)?;
        let mut buffer = &relayer_transaction_account.data.clone();
        let mut relayer_transaction = RelayerTransaction::deserialize(&mut buffer.as_slice())?;
        loop {
            match relayer_transaction {
                RelayerTransaction::Discovery(ref relayer_instruction) => {
                    let instruction = relayer_discovery.convert_instruction(relayer_instruction);
                    match instruction {
                        Ok(instruction) => {
                            let result = self.setup.mollusk.process_instruction(&instruction, &accounts);
                            match result.program_result {
                                ProgramResult::Success => {
                                    buffer = &result.return_data;
                                    relayer_transaction = RelayerTransaction::deserialize(&mut buffer.as_slice())?;
                                }
                                _ => {
                                    return Err(RelayerDiscoveryFixtureError::DiscoveryInstructionFailed(instruction, result.program_result));
                                }
                            }

                        },
                        Err(error) => {
                            match error {
                                ConvertError::NeedMessagePayload => {
                                    let account = (
                                        Pubkey::new_unique(),
                                        Account {
                                            lamports: 0,
                                            data: relayer_discovery.payload.clone(),
                                            owner: self.setup.payer.key(),
                                            executable: false,
                                            rent_epoch: 0,
                                        }
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
                                            owner: self.setup.payer.key(),
                                            executable: false,
                                            rent_epoch: 0,
                                        }
                                    );
                                    relayer_discovery.add_payer(account.0, amount);
                                    accounts.push(account);
                                }
                                _ => {
                                    return Err(error.into());
                                }
                            }
                        }
                    }

                },
                RelayerTransaction::Final(relayer_instructions) => {
                    return Ok(());
                },
            }
        }
        

    }

    pub fn approve_and_execute(&mut self, message: &Message, payload: Vec<u8>, accounts: Vec<(Pubkey, Account)>) -> Result<(), RelayerDiscoveryFixtureError> {
        let incoming_message_pda = self.approve(message);
        self.execute(message, incoming_message_pda, payload, accounts)
    }   
}