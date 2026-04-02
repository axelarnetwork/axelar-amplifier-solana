use std::collections::HashMap;

use anchor_lang::{InstructionData, ToAccountMetas};
use mollusk_svm::{result::Check, Mollusk, MolluskContext};
use mollusk_test_utils::get_event_authority_and_program_accounts;
use rand::Rng;
use solana_axelar_gateway::{
    state::config::{InitialVerifierSet, InitializeConfigParams},
    GatewayConfig, Message as CrossChainMessage, VerifierSetTracker,
};
use solana_axelar_std::{
    hasher::LeafHash, MerkleTree, MessageLeaf, PayloadType, PublicKey, Signature,
    SigningVerifierSetInfo, VerifierSetLeaf, U256,
};
use solana_sdk::{
    account::Account, instruction::Instruction, native_token::LAMPORTS_PER_SOL, pubkey::Pubkey,
};

use crate::{msg, TestHarness};

// -- Inlined from solana-axelar-gateway-test-fixtures --

pub fn generate_random_signer() -> (libsecp256k1::SecretKey, [u8; 33]) {
    let mut rng = rand::thread_rng();
    let secret_key_bytes: [u8; 32] = rng.gen();
    let secret_key = libsecp256k1::SecretKey::parse(&secret_key_bytes).unwrap();
    let public_key = libsecp256k1::PublicKey::from_secret_key(&secret_key);
    let compressed_pubkey = public_key.serialize_compressed();

    (secret_key, compressed_pubkey)
}

fn sign_message(
    message: &libsecp256k1::Message,
    secret_key: &libsecp256k1::SecretKey,
) -> Signature {
    let (sig, recovery_id) = libsecp256k1::sign(message, secret_key);
    let mut bytes = [0u8; 65];
    bytes[..64].copy_from_slice(&sig.serialize());
    bytes[64] = recovery_id.serialize();
    Signature(bytes)
}

pub fn create_verifier_info(
    secret_key: &libsecp256k1::SecretKey,
    payload_merkle_root: [u8; 32],
    verifier_leaf: &VerifierSetLeaf,
    position: usize,
    verifier_merkle_tree: &MerkleTree,
    payload_type: PayloadType,
) -> SigningVerifierSetInfo {
    let hashed_message = solana_axelar_std::execute_data::prefixed_message_hash_payload_type(
        payload_type,
        &payload_merkle_root,
    );

    let message = libsecp256k1::Message::parse(&hashed_message);
    let signature = sign_message(&message, secret_key);

    let merkle_proof = verifier_merkle_tree.proof(&[position]);
    let merkle_proof_bytes = merkle_proof.to_bytes();

    SigningVerifierSetInfo {
        signature,
        leaf: *verifier_leaf,
        merkle_proof: merkle_proof_bytes,
        payload_type,
    }
}

// -- Gateway harness info --

#[derive(Default)]
pub struct GatewayHarnessInfo {
    pub root: Pubkey,
    pub signers: Vec<libsecp256k1::SecretKey>,
    pub verifier_set_tracker: Pubkey,
    pub verifier_set_leaves: Vec<VerifierSetLeaf>,
    pub verifier_merkle_tree: MerkleTree,
}

// -- GatewaySetup trait (shared between GatewayTestHarness and ItsTestHarness) --

/// Trait for harnesses that manage gateway infrastructure (operators, gas service,
/// gateway, message approval). Provides default implementations so that any harness
/// with `payer`, `operator`, and `gateway` fields gets these methods for free.
pub trait GatewaySetup: TestHarness {
    fn payer(&self) -> Pubkey;
    fn operator(&self) -> Pubkey;
    fn gateway(&self) -> &GatewayHarnessInfo;
    fn gateway_mut(&mut self) -> &mut GatewayHarnessInfo;

    fn ensure_operators_initialized(&self) {
        let registry = solana_axelar_operators::OperatorRegistry::find_pda().0;
        let operator_account =
            solana_axelar_operators::OperatorAccount::find_pda(&self.operator()).0;

        if self.account_exists(&operator_account) {
            return;
        }

        let opr_init_ix = Instruction {
            program_id: solana_axelar_operators::ID,
            accounts: solana_axelar_operators::accounts::Initialize {
                payer: self.payer(),
                owner: self.operator(),
                registry,
                system_program: solana_sdk_ids::system_program::ID,
            }
            .to_account_metas(None),
            data: solana_axelar_operators::instruction::Initialize {}.data(),
        };

        let opr_add_operator_ix = Instruction {
            program_id: solana_axelar_operators::ID,
            accounts: solana_axelar_operators::accounts::AddOperator {
                owner: self.operator(),
                operator_to_add: self.operator(),
                registry,
                operator_account,
                system_program: solana_sdk_ids::system_program::ID,
            }
            .to_account_metas(None),
            data: solana_axelar_operators::instruction::AddOperator {}.data(),
        };

        self.ctx().process_and_validate_instruction_chain(&[
            (
                &opr_init_ix,
                &[
                    Check::success(),
                    Check::account(&registry)
                        .owner(&solana_axelar_operators::ID)
                        .rent_exempt()
                        .build(),
                ],
            ),
            (
                &opr_add_operator_ix,
                &[
                    Check::success(),
                    Check::account(&operator_account)
                        .owner(&solana_axelar_operators::ID)
                        .rent_exempt()
                        .build(),
                ],
            ),
        ]);

        msg!("Operators initialized.");
    }

    fn ensure_gas_service_initialized(&self) {
        self.ensure_operators_initialized();

        let treasury = solana_axelar_gas_service::Treasury::find_pda().0;
        let operator_account =
            solana_axelar_operators::OperatorAccount::find_pda(&self.operator()).0;

        if self.account_exists(&treasury) {
            return;
        }

        let gs_init_ix = Instruction {
            program_id: solana_axelar_gas_service::ID,
            accounts: solana_axelar_gas_service::accounts::Initialize {
                payer: self.payer(),
                operator: self.operator(),
                operator_pda: operator_account,
                system_program: solana_sdk_ids::system_program::ID,
                treasury,
            }
            .to_account_metas(None),
            data: solana_axelar_gas_service::instruction::Initialize {}.data(),
        };

        self.ctx().process_and_validate_instruction_chain(&[(
            &gs_init_ix,
            &[
                Check::success(),
                Check::account(&treasury)
                    .owner(&solana_axelar_gas_service::ID)
                    .rent_exempt()
                    .build(),
            ],
        )]);

        msg!("Gas service initialized.");
    }

    fn ensure_gateway_initialized(&mut self) {
        self.ensure_gas_service_initialized();

        let gateway_root_pda = GatewayConfig::find_pda().0;
        if self.account_exists(&gateway_root_pda) {
            return;
        }

        // Generate signers
        let (secret_key_1, compressed_pubkey_1) = generate_random_signer();
        let (secret_key_2, compressed_pubkey_2) = generate_random_signer();

        self.gateway_mut().signers = vec![secret_key_1, secret_key_2];

        let previous_verifier_retention = U256::from(5u64);
        let domain_separator = [2u8; 32];
        let minimum_rotation_delay = 3600;

        // Create verifier set
        let quorum_threshold = 100;
        let verifier_leaves = vec![
            VerifierSetLeaf {
                nonce: 0,
                quorum: quorum_threshold,
                signer_pubkey: PublicKey(compressed_pubkey_1),
                signer_weight: 50,
                position: 0,
                set_size: 2,
                domain_separator,
            },
            VerifierSetLeaf {
                nonce: 0,
                quorum: quorum_threshold,
                signer_pubkey: PublicKey(compressed_pubkey_2),
                signer_weight: 50,
                position: 1,
                set_size: 2,
                domain_separator,
            },
        ];

        self.gateway_mut()
            .verifier_set_leaves
            .clone_from(&verifier_leaves);

        let verifier_leaf_hashes: Vec<[u8; 32]> = verifier_leaves
            .iter()
            .map(solana_axelar_std::VerifierSetLeaf::hash)
            .collect();
        let verifier_merkle_tree = MerkleTree::from_leaves(&verifier_leaf_hashes);

        self.gateway_mut().verifier_merkle_tree = verifier_merkle_tree.clone();

        let verifier_set_hash = verifier_merkle_tree.root().unwrap();

        let verifier_set_tracker_pda = VerifierSetTracker::find_pda(&verifier_set_hash).0;

        let initial_verifier_set = InitialVerifierSet {
            hash: verifier_set_hash,
            pda: verifier_set_tracker_pda,
        };

        // Store program data account
        let program_data = self.ensure_program_data_account(
            "solana_axelar_gateway",
            &solana_axelar_gateway::ID,
            self.operator(),
        );

        // Initialize gateway
        let params = InitializeConfigParams {
            domain_separator,
            initial_verifier_set,
            minimum_rotation_delay,
            operator: self.operator(),
            previous_verifier_retention,
        };

        let gw_init_ix = Instruction {
            program_id: solana_axelar_gateway::ID,
            accounts: solana_axelar_gateway::accounts::InitializeConfig {
                payer: self.payer(),
                upgrade_authority: self.operator(),
                system_program: solana_sdk_ids::system_program::ID,
                program_data,
                gateway_root_pda,
                verifier_set_tracker_pda,
            }
            .to_account_metas(None),
            data: solana_axelar_gateway::instruction::InitializeConfig { params }.data(),
        };

        self.ctx().process_and_validate_instruction_chain(&[(
            &gw_init_ix,
            &[
                Check::success(),
                Check::account(&gateway_root_pda)
                    .owner(&solana_axelar_gateway::ID)
                    .rent_exempt()
                    .build(),
                Check::account(&verifier_set_tracker_pda)
                    .owner(&solana_axelar_gateway::ID)
                    .rent_exempt()
                    .build(),
            ],
        )]);

        self.gateway_mut().root = gateway_root_pda;
        self.gateway_mut().verifier_set_tracker = verifier_set_tracker_pda;

        msg!("Gateway initialized.");
    }

    #[allow(clippy::too_many_lines)]
    #[allow(clippy::cast_possible_truncation)]
    fn ensure_approved_incoming_messages(&self, messages: &[CrossChainMessage]) {
        let GatewayConfig {
            domain_separator, ..
        } = self
            .get_account_as(&self.gateway().root)
            .expect("gateway root should exist");

        // Merkle tree
        let message_leaves: Vec<MessageLeaf> = messages
            .iter()
            .enumerate()
            .map(|(i, msg)| MessageLeaf {
                message: msg.clone(),
                position: i as u16,
                set_size: messages.len() as u16,
                domain_separator,
            })
            .collect();

        let message_leaf_hashes: Vec<[u8; 32]> = message_leaves
            .iter()
            .map(solana_axelar_std::MessageLeaf::hash)
            .collect();

        let message_merkle_tree = MerkleTree::from_leaves(&message_leaf_hashes);

        let payload_merkle_root = message_merkle_tree.root().unwrap();

        let VerifierSetTracker {
            verifier_set_hash, ..
        } = self
            .get_account_as(&self.gateway().verifier_set_tracker)
            .expect("verifier set tracker should exist");

        // Initialize payload verification session
        let verification_session_account =
            solana_axelar_gateway::SignatureVerificationSessionData::find_pda(
                &payload_merkle_root,
                PayloadType::ApproveMessages,
                &verifier_set_hash,
            )
            .0;

        let init_session_ix = Instruction {
            program_id: solana_axelar_gateway::ID,
            accounts: solana_axelar_gateway::accounts::InitializePayloadVerificationSession {
                payer: self.payer(),
                gateway_root_pda: self.gateway().root,
                verifier_set_tracker_pda: self.gateway().verifier_set_tracker,
                verification_session_account,
                system_program: solana_sdk_ids::system_program::ID,
            }
            .to_account_metas(None),
            data: solana_axelar_gateway::instruction::InitializePayloadVerificationSession {
                merkle_root: payload_merkle_root,
                payload_type: PayloadType::ApproveMessages,
            }
            .data(),
        };

        let init_session_checks = vec![
            Check::success(),
            Check::account(&verification_session_account)
                .owner(&solana_axelar_gateway::ID)
                .rent_exempt()
                .build(),
        ];

        // Verifier info
        let verifier_infos = self
            .gateway()
            .signers
            .iter()
            .zip(self.gateway().verifier_set_leaves.iter())
            .enumerate()
            .map(|(idx, (sk, l))| {
                create_verifier_info(
                    sk,
                    payload_merkle_root,
                    l,
                    idx,
                    &self.gateway().verifier_merkle_tree,
                    PayloadType::ApproveMessages,
                )
            })
            .map(|verifier_info| Instruction {
                program_id: solana_axelar_gateway::ID,
                accounts: solana_axelar_gateway::accounts::VerifySignature {
                    gateway_root_pda: self.gateway().root,
                    verification_session_account,
                    verifier_set_tracker_pda: self.gateway().verifier_set_tracker,
                }
                .to_account_metas(None),
                data: solana_axelar_gateway::instruction::VerifySignature {
                    payload_merkle_root,
                    verifier_info,
                }
                .data(),
            })
            .collect::<Vec<_>>();

        let checks = vec![Check::success()];

        let verify_instruction_checks: Vec<(&Instruction, &[Check])> = verifier_infos
            .iter()
            .map(|ix| (ix, checks.as_slice()))
            .collect();

        // Approve messages
        let approve_message_ixs: Vec<Instruction> = messages
            .iter()
            .enumerate()
            .map(|(position, _msg)| {
                let message_proof = message_merkle_tree.proof(&[position]);
                let message_proof_bytes = message_proof.to_bytes();

                let merklized_message = solana_axelar_std::MerklizedMessage {
                    leaf: message_leaves[position].clone(),
                    proof: message_proof_bytes,
                };

                let command_id = messages[position].command_id();

                let incoming_message_pda =
                    solana_axelar_gateway::IncomingMessage::find_pda(&command_id).0;

                let (event_authority, _, _) =
                    get_event_authority_and_program_accounts(&solana_axelar_gateway::ID);

                Instruction {
                    program_id: solana_axelar_gateway::ID,
                    accounts: solana_axelar_gateway::accounts::ApproveMessage {
                        gateway_root_pda: self.gateway().root,
                        funder: self.payer(),
                        verification_session_account,
                        incoming_message_pda,
                        system_program: solana_sdk_ids::system_program::ID,
                        event_authority,
                        program: solana_axelar_gateway::ID,
                    }
                    .to_account_metas(None),
                    data: solana_axelar_gateway::instruction::ApproveMessage {
                        merklized_message,
                        payload_merkle_root,
                    }
                    .data(),
                }
            })
            .collect();

        let approve_checks = vec![Check::success()];
        let approve_instruction_checks: Vec<(&Instruction, &[Check])> = approve_message_ixs
            .iter()
            .map(|ix| (ix, approve_checks.as_slice()))
            .collect();

        // Execute all instructions
        let mut instruction_checks: Vec<(&Instruction, &[Check<'_>])> =
            vec![(&init_session_ix, &init_session_checks)];
        instruction_checks.extend(verify_instruction_checks);
        instruction_checks.extend(approve_instruction_checks);

        self.ctx()
            .process_and_validate_instruction_chain(&instruction_checks);

        msg!("Messages approved on gateway.");
    }
}

// -- Gateway test harness --

pub struct GatewayTestHarness {
    pub ctx: MolluskContext<HashMap<Pubkey, Account>>,
    pub payer: Pubkey,
    pub operator: Pubkey,
    pub gateway: GatewayHarnessInfo,
}

impl TestHarness for GatewayTestHarness {
    fn ctx(&self) -> &MolluskContext<HashMap<Pubkey, Account>> {
        &self.ctx
    }
}

impl GatewaySetup for GatewayTestHarness {
    fn payer(&self) -> Pubkey {
        self.payer
    }
    fn operator(&self) -> Pubkey {
        self.operator
    }
    fn gateway(&self) -> &GatewayHarnessInfo {
        &self.gateway
    }
    fn gateway_mut(&mut self) -> &mut GatewayHarnessInfo {
        &mut self.gateway
    }
}

impl Default for GatewayTestHarness {
    fn default() -> Self {
        let mollusk = initialize_gateway_mollusk();

        Self {
            ctx: mollusk.with_context(HashMap::new()),
            payer: Pubkey::new_unique(),
            operator: Pubkey::new_unique(),
            gateway: GatewayHarnessInfo::default(),
        }
    }
}

/// Creates a Mollusk instance with the gateway and its dependencies loaded.
pub fn initialize_gateway_mollusk() -> Mollusk {
    std::env::set_var("SBF_OUT_DIR", "../../target/deploy");
    let mut mollusk = Mollusk::new(&solana_axelar_gateway::ID, "solana_axelar_gateway");

    // Operators
    mollusk.add_program(
        &solana_axelar_operators::ID,
        "../../target/deploy/solana_axelar_operators",
    );

    // Gas Service
    mollusk.add_program(
        &solana_axelar_gas_service::ID,
        "../../target/deploy/solana_axelar_gas_service",
    );

    mollusk
}

impl GatewayTestHarness {
    pub fn new() -> Self {
        let mut harness = Self::default();

        harness.ensure_account_exists_with_lamports(harness.payer, LAMPORTS_PER_SOL * 100);
        harness.ensure_account_exists_with_lamports(harness.operator, LAMPORTS_PER_SOL * 100);
        harness.ensure_sysvar_instructions_account();
        harness.ensure_gateway_initialized();

        harness
    }
}
