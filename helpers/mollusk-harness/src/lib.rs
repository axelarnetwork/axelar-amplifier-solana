#![allow(clippy::indexing_slicing)]
use std::collections::HashMap;

use anchor_lang::{prelude::AccountMeta, InstructionData, ToAccountMetas};
use anchor_spl::{
    associated_token::{self, get_associated_token_address_with_program_id},
    token_2022::spl_token_2022,
};
use interchain_token_transfer_gmp::{GMPPayload, InterchainTransfer, ReceiveFromHub};
use mollusk_svm::{
    result::{Check, InstructionResult},
    MolluskContext,
};
use mollusk_test_utils::system_account_with_lamports;
use mollusk_test_utils::{create_program_data_account, get_event_authority_and_program_accounts};
use rand::Rng;
use solana_axelar_gateway_test_fixtures::create_verifier_info;
use solana_axelar_its::{
    instructions::{
        execute_interchain_transfer_extra_accounts, make_deploy_interchain_token_instruction,
    },
    InterchainTokenService,
};
use solana_axelar_its_test_fixtures::initialize_mollusk_with_programs;
use solana_axelar_std::{
    hasher::LeafHash, MerkleTree, MessageLeaf, PublicKey, VerifierSetLeaf, U256,
};
use solana_sdk::{
    account::Account, instruction::Instruction, native_token::LAMPORTS_PER_SOL, pubkey::Pubkey,
};

// Gateway
use solana_axelar_gateway::{
    state::config::{InitialVerifierSet, InitializeConfigParams},
    GatewayConfig, Message, VerifierSetTracker,
};

pub trait TestHarness {
    fn ctx(&self) -> &MolluskContext<HashMap<Pubkey, Account>>;

    fn account_exists(&self, address: &Pubkey) -> bool {
        self.ctx()
            .account_store
            .borrow()
            .get(address)
            .is_some_and(|acc| acc.lamports > 0)
    }

    fn store_account(&mut self, pubkey: Pubkey, account: Account) {
        self.ctx()
            .account_store
            .borrow_mut()
            .insert(pubkey, account);
    }

    // Get a cloned account from the context's account store.
    fn get_account(&self, address: &Pubkey) -> Option<Account> {
        self.ctx().account_store.borrow().get(address).cloned()
    }

    fn get_account_as<T: anchor_lang::AccountDeserialize>(&self, address: &Pubkey) -> Option<T> {
        let account = self.get_account(address)?;
        T::try_deserialize(&mut account.data.as_slice()).ok()
    }

    fn get_ata_2022_data(
        &self,
        wallet: Pubkey,
        token_mint: Pubkey,
    ) -> spl_token_2022::state::Account {
        let ata = get_associated_token_address_with_program_id(
            &wallet,
            &token_mint,
            &anchor_spl::token_2022::spl_token_2022::ID,
        );
        let ata = self
            .ctx()
            .account_store
            .borrow()
            .get(&ata)
            .expect("ata must exist")
            .clone();
        let ata =
            spl_token_2022::extension::StateWithExtensions::<spl_token_2022::state::Account>::unpack(
                &ata.data,
            ).expect("must be correct");
        ata.base
    }

    fn get_new_wallet(&self) -> Pubkey {
        let wallet = Pubkey::new_unique();
        self.ensure_account_exists_with_lamports(wallet, 10 * LAMPORTS_PER_SOL);
        wallet
    }

    /// Ensure an account exists in the context store with the given lamports.
    /// If the account does not exist, it will be created as a system account.
    /// However, this can be called on a non-system account (to be used for
    /// example when testing accidental nested owners).
    fn ensure_account_exists_with_lamports(&self, address: Pubkey, lamports: u64) {
        let mut store = self.ctx().account_store.borrow_mut();
        if let Some(existing) = store.get_mut(&address) {
            if existing.lamports < lamports {
                existing.lamports = lamports;
            }
        } else {
            store.insert(address, system_account_with_lamports(lamports));
        }
    }

    fn ensure_program_data_account(
        &mut self,
        name: &str,
        program: &Pubkey,
        upgrade_authority_address: Pubkey,
    ) -> Pubkey {
        let program_data = solana_sdk::bpf_loader_upgradeable::get_program_data_address(program);
        if self.account_exists(&program_data) {
            return program_data;
        }

        let program_elf = mollusk_svm::file::load_program_elf(name);
        let program_data_account =
            create_program_data_account(&program_elf, upgrade_authority_address);

        self.store_account(program_data, program_data_account);

        program_data
    }

    // TODO proper sysvar account is needed
    // See https://github.com/anza-xyz/mollusk/pull/170
    fn ensure_sysvar_instructions_account(&mut self) {
        use solana_sdk::sysvar::instructions::{construct_instructions_data, BorrowedInstruction};

        let sysvar_instructions_pubkey = solana_sdk::sysvar::instructions::id();
        if self.account_exists(&sysvar_instructions_pubkey) {
            return;
        }

        let instructions: &[BorrowedInstruction] = &[];

        let sysvar_account = Account {
            lamports: 1_000_000_000,
            data: construct_instructions_data(instructions),
            owner: solana_program::sysvar::id(),
            executable: false,
            rent_epoch: 0,
        };

        self.ctx()
            .account_store
            .borrow_mut()
            .insert(sysvar_instructions_pubkey, sysvar_account);
    }
}

pub struct ItsTestHarness {
    pub ctx: MolluskContext<HashMap<Pubkey, Account>>,

    pub payer: Pubkey,
    pub operator: Pubkey,

    // Gateway
    pub gateway: GatewayHarnessInfo,

    // ITS
    pub its_root: Pubkey,
}

impl TestHarness for ItsTestHarness {
    fn ctx(&self) -> &MolluskContext<HashMap<Pubkey, Account>> {
        &self.ctx
    }
}

pub struct GatewayHarnessInfo {
    pub root: Pubkey,
    pub signers: Vec<libsecp256k1::SecretKey>,
    pub verifier_set_tracker: Pubkey,
    pub verifier_set_leaves: Vec<VerifierSetLeaf>,
    pub verifier_merkle_tree: MerkleTree,
}

impl Default for ItsTestHarness {
    fn default() -> Self {
        Self::new()
    }
}

impl ItsTestHarness {
    pub fn new() -> Self {
        let mollusk = initialize_mollusk_with_programs();
        let payer = Pubkey::new_unique();
        let operator = Pubkey::new_unique();
        let ctx = mollusk.with_context(HashMap::new());

        let mut harness = Self {
            ctx,
            payer,
            operator,
            gateway: GatewayHarnessInfo {
                root: Pubkey::new_from_array([0u8; 32]),
                signers: vec![],
                verifier_set_tracker: Pubkey::new_from_array([0u8; 32]),
                verifier_set_leaves: vec![],
                verifier_merkle_tree: MerkleTree::new(),
            },
            its_root: Pubkey::new_from_array([0u8; 32]),
        };

        harness.ensure_account_exists_with_lamports(payer, LAMPORTS_PER_SOL * 100);
        harness.ensure_account_exists_with_lamports(operator, LAMPORTS_PER_SOL * 100);
        harness.ensure_its_initialized();
        harness.ensure_sysvar_instructions_account();

        harness
    }

    pub fn ensure_operators_initialized(&self) {
        let registry = solana_axelar_operators::OperatorRegistry::find_pda().0;
        let operator_account = solana_axelar_operators::OperatorAccount::find_pda(&self.operator).0;

        if self.account_exists(&operator_account) {
            return;
        }

        let opr_init_ix = Instruction {
            program_id: solana_axelar_operators::ID,
            accounts: solana_axelar_operators::accounts::Initialize {
                payer: self.payer,
                owner: self.operator,
                registry,
                system_program: solana_sdk::system_program::ID,
            }
            .to_account_metas(None),
            data: solana_axelar_operators::instruction::Initialize {}.data(),
        };

        let opr_add_operator_ix = Instruction {
            program_id: solana_axelar_operators::ID,
            accounts: solana_axelar_operators::accounts::AddOperator {
                owner: self.operator,
                operator_to_add: self.operator,
                registry,
                operator_account,
                system_program: solana_sdk::system_program::ID,
            }
            .to_account_metas(None),
            data: solana_axelar_operators::instruction::AddOperator {}.data(),
        };

        self.ctx.process_and_validate_instruction_chain(&[
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
    }

    pub fn ensure_gas_service_initialized(&self) {
        self.ensure_operators_initialized();

        let treasury = solana_axelar_gas_service::Treasury::find_pda().0;
        let operator_account = solana_axelar_operators::OperatorAccount::find_pda(&self.operator).0;

        if self.account_exists(&treasury) {
            return;
        }

        let gs_init_ix = Instruction {
            program_id: solana_axelar_gas_service::ID,
            accounts: solana_axelar_gas_service::accounts::Initialize {
                payer: self.payer,
                operator: self.operator,
                operator_pda: operator_account,
                system_program: solana_sdk::system_program::ID,
                treasury,
            }
            .to_account_metas(None),
            data: solana_axelar_gas_service::instruction::Initialize {}.data(),
        };

        self.ctx.process_and_validate_instruction_chain(&[(
            &gs_init_ix,
            &[
                Check::success(),
                Check::account(&treasury)
                    .owner(&solana_axelar_gas_service::ID)
                    .rent_exempt()
                    .build(),
            ],
        )]);
    }

    // TODO move to gateway harness
    pub fn ensure_gateway_initialized(&mut self) {
        self.ensure_gas_service_initialized();

        let gateway_root_pda = solana_axelar_gateway::GatewayConfig::find_pda().0;
        if self.account_exists(&gateway_root_pda) {
            return;
        }

        // Gateway init params

        // Generate signers
        let (secret_key_1, compressed_pubkey_1) =
            solana_axelar_gateway_test_fixtures::generate_random_signer();
        let (secret_key_2, compressed_pubkey_2) =
            solana_axelar_gateway_test_fixtures::generate_random_signer();

        self.gateway.signers = vec![secret_key_1, secret_key_2];

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

        self.gateway
            .verifier_set_leaves
            .clone_from(&verifier_leaves);

        // Calculate the verifier set hash
        let verifier_leaf_hashes: Vec<[u8; 32]> = verifier_leaves
            .iter()
            .map(solana_axelar_std::VerifierSetLeaf::hash)
            .collect();
        let verifier_merkle_tree = MerkleTree::from_leaves(&verifier_leaf_hashes);

        self.gateway.verifier_merkle_tree = verifier_merkle_tree.clone();

        let verifier_set_hash = verifier_merkle_tree.root().unwrap();

        let verifier_set_tracker_pda =
            solana_axelar_gateway::VerifierSetTracker::find_pda(&verifier_set_hash).0;

        let initial_verifier_set = InitialVerifierSet {
            hash: verifier_set_hash,
            pda: verifier_set_tracker_pda,
        };

        // Store accounts

        let program_data = self.ensure_program_data_account(
            "solana_axelar_gateway",
            &solana_axelar_gateway::ID,
            self.operator,
        );

        // Initialize gateway

        let params = InitializeConfigParams {
            domain_separator,
            initial_verifier_set,
            minimum_rotation_delay,
            operator: self.operator,
            previous_verifier_retention,
        };

        let gw_init_ix = Instruction {
            program_id: solana_axelar_gateway::ID,
            accounts: solana_axelar_gateway::accounts::InitializeConfig {
                payer: self.payer,
                upgrade_authority: self.operator,
                system_program: solana_sdk::system_program::ID,
                program_data,
                gateway_root_pda,
                verifier_set_tracker_pda,
            }
            .to_account_metas(None),
            data: solana_axelar_gateway::instruction::InitializeConfig { params }.data(),
        };

        self.ctx.process_and_validate_instruction_chain(&[(
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

        self.gateway.root = gateway_root_pda;
        self.gateway.verifier_set_tracker = verifier_set_tracker_pda;
    }

    #[allow(clippy::too_many_lines)]
    #[allow(clippy::cast_possible_truncation)]
    pub fn ensure_approved_incoming_messages(&self, messages: &[Message]) {
        let GatewayConfig {
            domain_separator, ..
        } = self
            .get_account_as(&self.gateway.root)
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
            .get_account_as(&self.gateway.verifier_set_tracker)
            .expect("verifier set tracker should exist");

        // Initialize payload verification session
        // TODO: extract to helper

        let verification_session_account =
            solana_axelar_gateway::SignatureVerificationSessionData::find_pda(
                &payload_merkle_root,
                &verifier_set_hash,
            )
            .0;

        let init_session_ix = Instruction {
            program_id: solana_axelar_gateway::ID,
            accounts: solana_axelar_gateway::accounts::InitializePayloadVerificationSession {
                payer: self.payer,
                gateway_root_pda: self.gateway.root,
                verifier_set_tracker_pda: self.gateway.verifier_set_tracker,
                verification_session_account,
                system_program: solana_sdk::system_program::ID,
            }
            .to_account_metas(None),
            data: solana_axelar_gateway::instruction::InitializePayloadVerificationSession {
                merkle_root: payload_merkle_root,
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
            .gateway
            .signers
            .iter()
            .zip(self.gateway.verifier_set_leaves.iter())
            .enumerate()
            .map(|(idx, (sk, l))| {
                create_verifier_info(
                    sk,
                    payload_merkle_root,
                    l,
                    idx,
                    &self.gateway.verifier_merkle_tree,
                )
            })
            .map(|verifier_info| Instruction {
                program_id: solana_axelar_gateway::ID,
                accounts: solana_axelar_gateway::accounts::VerifySignature {
                    gateway_root_pda: self.gateway.root,
                    verification_session_account,
                    verifier_set_tracker_pda: self.gateway.verifier_set_tracker,
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
                        gateway_root_pda: self.gateway.root,
                        funder: self.payer,
                        verification_session_account,
                        incoming_message_pda,
                        system_program: solana_sdk::system_program::ID,
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

        self.ctx
            .process_and_validate_instruction_chain(&instruction_checks);

        solana_sdk::msg!("Messages approved on gateway.");
    }

    pub fn ensure_its_initialized(&mut self) {
        self.ensure_gateway_initialized();

        let its_root_pda = solana_axelar_its::InterchainTokenService::find_pda().0;
        if self.account_exists(&its_root_pda) {
            return;
        }

        let program_data = self.ensure_program_data_account(
            "solana_axelar_its",
            &solana_axelar_its::ID,
            self.operator,
        );

        let user_roles_pda =
            solana_axelar_its::UserRoles::find_pda(&its_root_pda, &self.operator).0;

        let its_init_ix = Instruction {
            program_id: solana_axelar_its::ID,
            accounts: solana_axelar_its::accounts::Initialize {
                payer: self.operator,
                program_data,
                its_root_pda,
                system_program: solana_sdk::system_program::ID,
                operator: self.operator,
                user_roles_account: user_roles_pda,
            }
            .to_account_metas(None),
            data: solana_axelar_its::instruction::Initialize {
                chain_name: "solana".to_owned(),
                its_hub_address: "axelar123".to_owned(),
            }
            .data(),
        };

        self.ctx.process_and_validate_instruction_chain(&[(
            &its_init_ix,
            &[
                Check::success(),
                Check::account(&its_root_pda)
                    .owner(&solana_axelar_its::ID)
                    .rent_exempt()
                    .build(),
                Check::account(&user_roles_pda)
                    .owner(&solana_axelar_its::ID)
                    .rent_exempt()
                    .build(),
            ],
        )]);

        self.its_root = its_root_pda;
    }

    pub fn ensure_trusted_chain(&mut self, trusted_chain_name: &str) {
        self.ensure_its_initialized();

        let program_data = self.ensure_program_data_account(
            "solana_axelar_its",
            &solana_axelar_its::ID,
            self.operator,
        );

        let (event_authority, _, _) =
            get_event_authority_and_program_accounts(&solana_axelar_its::ID);

        let ix = Instruction {
            program_id: solana_axelar_its::ID,
            accounts: solana_axelar_its::accounts::SetTrustedChain {
                payer: self.operator,
                program_data: Some(program_data),
                user_roles: None,
                its_root_pda: self.its_root,
                system_program: solana_sdk::system_program::ID,
                // Event authority
                event_authority,
                // The current program account
                program: solana_axelar_its::ID,
            }
            .to_account_metas(None),
            data: solana_axelar_its::instruction::SetTrustedChain {
                chain_name: trusted_chain_name.to_owned(),
            }
            .data(),
        };

        self.ctx
            .process_and_validate_instruction_chain(&[(&ix, &[Check::success()])]);

        let its: solana_axelar_its::InterchainTokenService = self
            .get_account_as(&self.its_root)
            .expect("ITS root account should exist");

        assert_eq!(its.trusted_chains.len(), 1, "must have one trusted chain");
        assert!(
            its.trusted_chains.iter().any(|x| x == trusted_chain_name),
            "must have the trusted chain added"
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub fn ensure_deploy_local_interchain_token(
        &self,
        deployer: Pubkey,
        salt: [u8; 32],
        name: String,
        symbol: String,
        decimals: u8,
        initial_supply: u64,
        minter: Option<Pubkey>,
    ) -> [u8; 32] {
        let token_id = solana_axelar_its::utils::interchain_token_id(&deployer, &salt);
        let token_manager_pda =
            solana_axelar_its::TokenManager::find_pda(token_id, self.its_root).0;

        if self.account_exists(&token_manager_pda) {
            return token_id;
        }

        let (deploy_ix, deploy_accounts) = make_deploy_interchain_token_instruction(
            self.payer,
            deployer,
            salt,
            name,
            symbol,
            decimals,
            initial_supply,
            minter,
        );

        solana_sdk::msg!(
            "Deploying interchain token with ID: {}",
            hex::encode(token_id),
        );

        self.ctx.process_and_validate_instruction_chain(&[(
            &deploy_ix,
            &[
                Check::success(),
                Check::account(&token_manager_pda)
                    .owner(&solana_axelar_its::ID)
                    .rent_exempt()
                    .build(),
                Check::account(&deploy_accounts.token_mint)
                    .owner(&spl_token_2022::ID)
                    .rent_exempt()
                    .build(),
            ],
        )]);

        let token_manager: solana_axelar_its::TokenManager = self
            .get_account_as(&token_manager_pda)
            .expect("token manager account should exist");

        solana_sdk::msg!(
            "Deployed interchain token mint: {}",
            token_manager.token_address,
        );

        token_id
    }

    pub fn ensure_test_interchain_token(&self) -> [u8; 32] {
        let salt = [1u8; 32];
        let name = "Test Token".to_owned();
        let symbol = "TTK".to_owned();
        let decimals = 9u8;
        let initial_supply = 1_000_000_000_000; // 1,000 TTK
        let minter = Some(self.operator);

        self.ensure_deploy_local_interchain_token(
            self.operator,
            salt,
            name,
            symbol,
            decimals,
            initial_supply,
            minter,
        )
    }

    pub fn execute_gmp(
        &self,
        token_id: [u8; 32],
        source_chain: &str,
        payload: GMPPayload,
        extra_accounts: Vec<AccountMeta>,
    ) -> InstructionResult {
        let encoded_payload = payload.encode();
        let payload_hash = solana_sdk::keccak::hashv(&[&encoded_payload]).to_bytes();

        let rand_message_id: String = rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(32)
            .map(char::from)
            .collect();

        let InterchainTokenService {
            its_hub_address, ..
        } = self
            .get_account_as(&self.its_root)
            .expect("its config should exist");

        let message = solana_axelar_gateway::Message {
            cc_id: solana_axelar_std::CrossChainId {
                chain: source_chain.to_owned(),
                id: rand_message_id,
            },
            source_address: its_hub_address,
            destination_chain: "solana".to_owned(),
            destination_address: solana_axelar_its::ID.to_string(),
            payload_hash,
        };

        self.ensure_approved_incoming_messages(&[message.clone()]);

        let incoming_message_pda =
            solana_axelar_gateway::IncomingMessage::find_pda(&message.command_id()).0;
        let incoming_message = self
            .get_account_as::<solana_axelar_gateway::IncomingMessage>(&incoming_message_pda)
            .expect("incoming message account should exist");

        let token_manager_pda =
            solana_axelar_its::TokenManager::find_pda(token_id, self.its_root).0;
        let token_mint =
            solana_axelar_its::TokenManager::find_token_mint(token_id, self.its_root).0;
        let token_manager_ata = get_associated_token_address_with_program_id(
            &token_manager_pda,
            &token_mint,
            &spl_token_2022::ID,
        );

        // TODO extract to helper
        let executable = solana_axelar_its::accounts::AxelarExecuteAccounts {
            incoming_message_pda,
            signing_pda: Pubkey::create_program_address(
                &[
                    solana_axelar_gateway::seed_prefixes::VALIDATE_MESSAGE_SIGNING_SEED,
                    message.command_id().as_ref(),
                    &[incoming_message.signing_pda_bump],
                ],
                &solana_axelar_its::ID,
            )
            .expect("valid signing PDA"),
            gateway_root_pda: self.gateway.root,
            event_authority: get_event_authority_and_program_accounts(&solana_axelar_gateway::ID).0,
            axelar_gateway_program: solana_axelar_gateway::ID,
        };

        let mut accounts = solana_axelar_its::accounts::Execute {
            executable,
            payer: self.payer,
            system_program: solana_sdk::system_program::ID,
            its_root_pda: self.its_root,
            token_mint,
            token_manager_pda,
            token_manager_ata,
            token_program: spl_token_2022::ID,
            associated_token_program: associated_token::ID,
            event_authority: get_event_authority_and_program_accounts(&solana_axelar_its::ID).0,
            program: solana_axelar_its::ID,
        }
        .to_account_metas(None);
        accounts.extend(extra_accounts);

        let ix = Instruction {
            program_id: solana_axelar_its::ID,
            accounts,
            data: solana_axelar_its::instruction::Execute {
                message,
                payload: encoded_payload,
            }
            .data(),
        };

        self.ctx
            .process_and_validate_instruction_chain(&[(&ix, &[Check::success()])])
    }

    pub fn execute_gmp_transfer(
        &self,
        token_id: [u8; 32],
        source_chain: &str,
        source_address: &str,
        destination_address: Pubkey,
        amount: u64,
        data: Option<(Vec<u8>, Vec<AccountMeta>)>,
    ) -> InstructionResult {
        let has_data = data.is_some();

        let transfer_payload = InterchainTransfer {
            selector: InterchainTransfer::MESSAGE_TYPE_ID_UINT,
            token_id: alloy_primitives::FixedBytes::from(token_id),
            source_address: alloy_primitives::Bytes::from(source_address.as_bytes().to_vec()),
            destination_address: alloy_primitives::Bytes::from(
                destination_address.to_bytes().to_vec(),
            ),
            amount: alloy_primitives::U256::from(amount),
            data: data
                .as_ref()
                .map_or(alloy_primitives::Bytes::new(), |(d, _)| d.clone().into()),
        };

        let token_mint =
            solana_axelar_its::TokenManager::find_token_mint(token_id, self.its_root).0;

        let destination_ata = get_associated_token_address_with_program_id(
            &destination_address,
            &token_mint,
            &spl_token_2022::ID,
        );

        let mut extra_accounts = execute_interchain_transfer_extra_accounts(
            destination_address,
            destination_ata,
            Some(has_data),
        );
        if let Some((_, data_accounts)) = data {
            extra_accounts.extend(data_accounts);
        }

        let transfer_payload_wrapped = GMPPayload::ReceiveFromHub(ReceiveFromHub {
            selector: ReceiveFromHub::MESSAGE_TYPE_ID_UINT,
            source_chain: source_chain.to_owned(),
            payload: GMPPayload::InterchainTransfer(transfer_payload)
                .encode()
                .into(),
        });

        self.execute_gmp(
            token_id,
            source_chain,
            transfer_payload_wrapped,
            extra_accounts,
        )
    }
}
