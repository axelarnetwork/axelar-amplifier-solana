#![allow(clippy::indexing_slicing)]
#![allow(clippy::too_many_arguments)]
use std::collections::HashMap;

use anchor_lang::{InstructionData, ToAccountMetas};
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
use relayer_discovery_test_fixtures::relayer_execute_with_checks;
use solana_axelar_gateway_test_fixtures::create_verifier_info;
use solana_axelar_its::{
    instructions::{
        make_deploy_interchain_token_instruction, make_interchain_transfer_instruction,
        make_mint_interchain_token_instruction, make_register_canonical_token_instruction,
        make_set_trusted_chain_instruction,
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

macro_rules! msg {
    () => {
        solana_sdk::msg!("[mollusk-harness]");
    };
    ($msg:literal) => {
        solana_sdk::msg!(concat!("[mollusk-harness] ", $msg));
    };
    ($fmt:literal, $($arg:tt)*) => {
        solana_sdk::msg!(concat!("[mollusk-harness] ", $fmt), $($arg)*);
    };
}

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

    /// Creates a native SPL Token 2022 mint and stores it in the context.
    /// Returns the mint pubkey.
    fn create_spl_token_mint(
        &self,
        mint_authority: Pubkey,
        decimals: u8,
        supply: Option<u64>,
    ) -> Pubkey {
        use solana_sdk::program_pack::Pack;

        let mint = Pubkey::new_unique();
        let mint_data = {
            let mut data = [0u8; spl_token_2022::state::Mint::LEN];
            let mint_state = spl_token_2022::state::Mint {
                mint_authority: Some(mint_authority).into(),
                supply: supply.unwrap_or(1_000_000_000),
                decimals,
                is_initialized: true,
                freeze_authority: Some(mint_authority).into(),
            };
            spl_token_2022::state::Mint::pack(mint_state, &mut data).unwrap();
            data.to_vec()
        };

        let rent = solana_sdk::rent::Rent::default();
        let mint_account = Account {
            lamports: rent.minimum_balance(mint_data.len()),
            data: mint_data,
            owner: spl_token_2022::ID,
            executable: false,
            rent_epoch: 0,
        };

        self.ctx()
            .account_store
            .borrow_mut()
            .insert(mint, mint_account);

        msg!("Created SPL Token 2022 mint: {}", mint);

        mint
    }

    /// Creates a native SPL Token 2022 mint with transfer fee extension using real instructions.
    /// Returns the mint pubkey.
    fn create_spl_token_mint_with_transfer_fee(
        &self,
        mint_authority: Pubkey,
        decimals: u8,
        transfer_fee_basis_points: u16,
        maximum_fee: u64,
    ) -> Pubkey {
        use spl_token_2022::extension::ExtensionType;
        use spl_token_2022::instruction as token_instruction;

        let mint = Pubkey::new_unique();

        // Calculate space needed for mint with transfer fee extension
        let extension_types = &[ExtensionType::TransferFeeConfig];
        let space = ExtensionType::try_calculate_account_len::<spl_token_2022::state::Mint>(
            extension_types,
        )
        .unwrap();

        let rent = solana_sdk::rent::Rent::default();
        let lamports = rent.minimum_balance(space);

        // Pre-create the account with correct size
        let mint_account = Account {
            lamports,
            data: vec![0u8; space],
            owner: spl_token_2022::ID,
            executable: false,
            rent_epoch: 0,
        };

        self.ctx()
            .account_store
            .borrow_mut()
            .insert(mint, mint_account);

        // 1. Initialize transfer fee config (must happen before mint init)
        let init_fee_ix =
            spl_token_2022::extension::transfer_fee::instruction::initialize_transfer_fee_config(
                &spl_token_2022::ID,
                &mint,
                Some(&mint_authority),
                Some(&mint_authority),
                transfer_fee_basis_points,
                maximum_fee,
            )
            .unwrap();

        // 2. Initialize the mint
        let init_mint_ix = token_instruction::initialize_mint2(
            &spl_token_2022::ID,
            &mint,
            &mint_authority,
            Some(&mint_authority),
            decimals,
        )
        .unwrap();

        // Execute both instructions
        self.ctx().process_and_validate_instruction_chain(&[
            (&init_fee_ix, &[Check::success()]),
            (&init_mint_ix, &[Check::success()]),
        ]);

        msg!(
            "Created SPL Token 2022 mint with transfer fee: {} (fee: {} bps, max: {})",
            mint,
            transfer_fee_basis_points,
            maximum_fee
        );

        mint
    }

    /// Get a token account (legacy or 2022) from the context's account store.
    fn get_token_account(
        &self,
        address: &Pubkey,
    ) -> Option<anchor_spl::token_interface::TokenAccount> {
        self.get_account_as(address)
    }

    fn update_account<F>(&mut self, address: &Pubkey, updater: F)
    where
        F: FnOnce(&mut Account),
    {
        let mut account = self.get_account(address).expect("account should exist");
        updater(&mut account);
        self.store_account(*address, account);
    }

    fn update_account_as<T, F>(&mut self, address: &Pubkey, updater: F) -> T
    where
        T: anchor_lang::AccountDeserialize
            + anchor_lang::AnchorSerialize
            + anchor_lang::Discriminator,
        F: FnOnce(&mut T),
    {
        let mut account = self.get_account(address).expect("account should exist");
        let mut data = T::try_deserialize(&mut account.data.as_slice())
            .expect("failed to deserialize account");

        updater(&mut data);

        data.serialize(&mut &mut account.data[T::DISCRIMINATOR.len()..])
            .expect("failed to serialize account");

        self.store_account(*address, account);

        data
    }

    fn get_ata_2022_address(&self, wallet: Pubkey, token_mint: Pubkey) -> Pubkey {
        get_associated_token_address_with_program_id(
            &wallet,
            &token_mint,
            &anchor_spl::token_2022::spl_token_2022::ID,
        )
    }

    /// For when we manually need to check rent exemption.
    fn is_rent_exempt(&self, address: &Pubkey) -> bool {
        let account = self
            .get_account(address)
            .expect("account must exist to check rent exemption");
        self.ctx()
            .mollusk
            .sysvars
            .rent
            .is_exempt(account.lamports, account.data.len())
    }

    fn assert_rent_exempt(&self, address: &Pubkey) {
        assert!(
            self.is_rent_exempt(address),
            "account {address} is not rent exempt",
        );
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

    fn get_or_create_ata_2022_account(
        &self,
        payer: Pubkey,
        wallet: Pubkey,
        token_mint: Pubkey,
    ) -> (Pubkey, spl_token_2022::state::Account) {
        let ata = get_associated_token_address_with_program_id(
            &wallet,
            &token_mint,
            &anchor_spl::token_2022::spl_token_2022::ID,
        );

        if !self.account_exists(&ata) {
            let create_ata_ix =
                associated_token::spl_associated_token_account::instruction::create_associated_token_account(
                    &payer,
                    &wallet,
                    &token_mint,
                    &anchor_spl::token_2022::spl_token_2022::ID,
                );

            self.ctx()
                .process_and_validate_instruction(&create_ata_ix, &[Check::success()]);

            msg!(
                "Created ATA account for wallet: {}, mint: {}",
                wallet,
                token_mint
            );
        }

        let ata_data = self.get_ata_2022_data(wallet, token_mint);

        (ata, ata_data)
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

    /// Creates metadata for a token mint and stores it in the context.
    /// Returns the metadata PDA.
    fn create_token_metadata(
        &self,
        mint: Pubkey,
        mint_authority: Pubkey,
        name: String,
        symbol: String,
    ) -> Pubkey {
        use anchor_lang::AnchorSerialize;

        let (metadata_pda, _) = mpl_token_metadata::accounts::Metadata::find_pda(&mint);

        let uri = format!("https://{}.com", symbol.to_lowercase());

        let metadata = mpl_token_metadata::accounts::Metadata {
            key: mpl_token_metadata::types::Key::MetadataV1,
            update_authority: mint_authority,
            mint,
            name,
            symbol,
            uri,
            seller_fee_basis_points: 0,
            creators: None,
            primary_sale_happened: false,
            is_mutable: true,
            edition_nonce: None,
            token_standard: Some(mpl_token_metadata::types::TokenStandard::Fungible),
            collection: None,
            uses: None,
            collection_details: None,
            programmable_config: None,
        };

        let metadata_data = metadata.try_to_vec().unwrap();
        let rent = solana_sdk::rent::Rent::default();
        let metadata_account = Account {
            lamports: rent.minimum_balance(metadata_data.len()),
            data: metadata_data,
            owner: mpl_token_metadata::programs::MPL_TOKEN_METADATA_ID,
            executable: false,
            rent_epoch: 0,
        };

        self.ctx()
            .account_store
            .borrow_mut()
            .insert(metadata_pda, metadata_account);

        msg!("Created token metadata for mint: {}", mint);

        metadata_pda
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

#[derive(Default)]
pub struct GatewayHarnessInfo {
    pub root: Pubkey,
    pub signers: Vec<libsecp256k1::SecretKey>,
    pub verifier_set_tracker: Pubkey,
    pub verifier_set_leaves: Vec<VerifierSetLeaf>,
    pub verifier_merkle_tree: MerkleTree,
}

impl Default for ItsTestHarness {
    /// Create a default ITS test harness without initializing ITS.
    /// Useful for testing initialization itself.
    fn default() -> Self {
        let mollusk = initialize_mollusk_with_programs();

        Self {
            ctx: mollusk.with_context(HashMap::new()),
            payer: Pubkey::new_unique(),
            operator: Pubkey::new_unique(),
            gateway: GatewayHarnessInfo::default(),
            its_root: Pubkey::default(),
        }
    }
}

impl ItsTestHarness {
    pub fn new() -> Self {
        let mut harness = Self::default();

        harness.ensure_account_exists_with_lamports(harness.payer, LAMPORTS_PER_SOL * 100);
        harness.ensure_account_exists_with_lamports(harness.operator, LAMPORTS_PER_SOL * 100);
        harness.ensure_sysvar_instructions_account();
        harness.ensure_its_initialized();

        harness
    }

    pub fn get_its_root(&self) -> InterchainTokenService {
        self.get_account_as(&self.its_root)
            .expect("ITS root account should exist")
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
                system_program: solana_sdk_ids::system_program::ID,
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
                system_program: solana_sdk_ids::system_program::ID,
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

        msg!("Operators initialized.");
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
                system_program: solana_sdk_ids::system_program::ID,
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

        msg!("Gas service initialized.");
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
                system_program: solana_sdk_ids::system_program::ID,
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

        msg!("Gateway initialized.");
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
                system_program: solana_sdk_ids::system_program::ID,
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

        self.ctx
            .process_and_validate_instruction_chain(&instruction_checks);

        msg!("Messages approved on gateway.");
    }

    pub fn ensure_its_initialized(&mut self) {
        self.ensure_gateway_initialized();

        let its_root_pda = solana_axelar_its::InterchainTokenService::find_pda().0;
        if self.account_exists(&its_root_pda) {
            return;
        }

        // operator will also serve as the payer and upgrade authority
        let upgrade_authority = self.operator;

        self.ensure_program_data_account(
            "solana_axelar_its",
            &solana_axelar_its::ID,
            upgrade_authority,
        );

        let (init_ix, init_accounts) = solana_axelar_its::instructions::make_initialize_instruction(
            upgrade_authority,
            self.operator,
            "solana".to_owned(),
            "axelar123".to_owned(),
        );

        self.ctx.process_and_validate_instruction(
            &init_ix,
            &[
                Check::success(),
                Check::account(&its_root_pda)
                    .owner(&solana_axelar_its::ID)
                    .rent_exempt()
                    .build(),
                Check::account(&init_accounts.user_roles_account)
                    .owner(&solana_axelar_its::ID)
                    .rent_exempt()
                    .build(),
            ],
        );

        self.its_root = its_root_pda;
    }

    /// Shortcut function to get the token mint for a given token ID.
    pub fn token_mint_for_id(&self, token_id: [u8; 32]) -> Pubkey {
        solana_axelar_its::TokenManager::find_token_mint(token_id, self.its_root).0
    }

    pub fn ensure_trusted_chain(&mut self, trusted_chain_name: &str) {
        let its = self.get_its_root();
        let trusted_chains_before = its.trusted_chains.len();

        msg!("Ensuring trusted chain: {}", trusted_chain_name);

        let ix =
            make_set_trusted_chain_instruction(self.operator, trusted_chain_name.to_owned(), false)
                .0;

        self.ctx.process_and_validate_instruction(
            &ix,
            &[
                Check::success(),
                Check::account(&self.its_root).rent_exempt().build(),
            ],
        );

        let its = self.get_its_root();

        assert_eq!(
            its.trusted_chains.len(),
            trusted_chains_before + 1,
            "must have the trusted chain appended"
        );
        assert!(
            its.trusted_chains.iter().any(|x| x == trusted_chain_name),
            "must have the trusted chain added"
        );
    }

    pub fn ensure_transfer_operatorship(&mut self, new_operator: Pubkey) {
        let ix = solana_axelar_its::instructions::make_transfer_operatorship_instruction(
            self.payer,
            self.operator,
            new_operator,
        )
        .0;

        let old_operator_account =
            solana_axelar_its::UserRoles::find_pda(&self.its_root, &self.operator).0;

        let old_its_roles: solana_axelar_its::UserRoles = self
            .get_account_as(&old_operator_account)
            .expect("old operator roles account should exist before transfer");

        // Check if old operator only had operator role
        let should_be_closed = old_its_roles.roles == solana_axelar_its::Roles::OPERATOR;

        // Process
        self.ctx
            .process_and_validate_instruction(&ix, &[Check::success()]);

        // Check old operator
        {
            let old_its_roles: Option<solana_axelar_its::UserRoles> =
                self.get_account_as(&old_operator_account);

            if should_be_closed {
                assert!(
                    old_its_roles.is_none(),
                    "old operator roles account should be closed"
                );
            } else {
                assert!(
                    !old_its_roles
                        .expect("old operator account should still have other roles")
                        .has_operator_role(),
                    "old operator must not have operator role"
                );
            }
        }

        // Check new operator
        {
            let new_operator_account =
                solana_axelar_its::UserRoles::find_pda(&self.its_root, &new_operator).0;

            let its_roles: solana_axelar_its::UserRoles = self
                .get_account_as(&new_operator_account)
                .expect("new operator roles account should exist");

            assert!(
                its_roles.has_operator_role(),
                "new operator must have operator role"
            );
        }

        // Update operator in harness
        self.operator = new_operator;
    }

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

        msg!(
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

        msg!(
            "Deployed interchain token mint: {}",
            token_manager.token_address,
        );

        token_id
    }

    /// Registers a canonical token (existing SPL token) with the ITS.
    /// Returns the token_id for the registered canonical token.
    pub fn ensure_register_canonical_token(&self, token_mint: Pubkey) -> [u8; 32] {
        let token_id = solana_axelar_its::utils::canonical_interchain_token_id(&token_mint);
        let token_manager_pda =
            solana_axelar_its::TokenManager::find_pda(token_id, self.its_root).0;

        if self.account_exists(&token_manager_pda) {
            return token_id;
        }

        let (ix, accounts) =
            make_register_canonical_token_instruction(self.payer, token_mint, spl_token_2022::ID);

        msg!(
            "Registering canonical token {} with ID: {}",
            token_mint,
            hex::encode(token_id),
        );

        let expected_token_id =
            solana_axelar_its::utils::canonical_interchain_token_id(&token_mint);

        self.ctx.process_and_validate_instruction_chain(&[(
            &ix,
            &[
                Check::success(),
                Check::return_data(&expected_token_id),
                Check::account(&accounts.token_manager_pda)
                    .owner(&solana_axelar_its::ID)
                    .rent_exempt()
                    .build(),
            ],
        )]);

        token_id
    }

    pub const TEST_TOKEN_SALT: [u8; 32] = [1u8; 32];
    pub const TEST_TOKEN_NAME: &'static str = "Test Token";
    pub const TEST_TOKEN_SYMBOL: &'static str = "TTK";
    pub const TEST_TOKEN_DECIMALS: u8 = 9;
    pub const TEST_TOKEN_INITIAL_SUPPLY: u64 = 1_000_000_000_000; // 1,000 TTK

    #[must_use]
    pub fn ensure_test_interchain_token(&self) -> [u8; 32] {
        msg!("Deploying test local interchain token.");

        let minter = Some(self.operator);

        self.ensure_deploy_local_interchain_token(
            self.operator,
            Self::TEST_TOKEN_SALT,
            Self::TEST_TOKEN_NAME.to_owned(),
            Self::TEST_TOKEN_SYMBOL.to_owned(),
            Self::TEST_TOKEN_DECIMALS,
            Self::TEST_TOKEN_INITIAL_SUPPLY,
            minter,
        )
    }

    pub const TEST_CANONICAL_TOKEN_NAME: &'static str = "Canonical Token";
    pub const TEST_CANONICAL_TOKEN_SYMBOL: &'static str = "CTKN";
    pub const TEST_CANONICAL_TOKEN_DECIMALS: u8 = 9;
    pub const TEST_CANONICAL_TOKEN_INITIAL_SUPPLY: u64 = 1_000_000_000_000; // 1,000 TTK

    #[must_use]
    pub fn ensure_test_registered_canonical_token(
        &self,
        mint_authority: Pubkey,
    ) -> (Pubkey, [u8; 32]) {
        let token_mint = self.create_spl_token_mint(
            mint_authority,
            Self::TEST_CANONICAL_TOKEN_DECIMALS,
            Some(Self::TEST_CANONICAL_TOKEN_INITIAL_SUPPLY),
        );

        self.create_token_metadata(
            token_mint,
            mint_authority,
            Self::TEST_CANONICAL_TOKEN_NAME.to_owned(),
            Self::TEST_CANONICAL_TOKEN_SYMBOL.to_owned(),
        );

        let token_id = self.ensure_register_canonical_token(token_mint);

        (token_mint, token_id)
    }

    pub fn ensure_mint_interchain_token(
        &self,
        token_id: [u8; 32],
        amount: u64,
        minter: Pubkey,
        destination_account: Pubkey,
        token_program: Pubkey,
    ) {
        // Check balance before minting
        let dest: Option<anchor_spl::token_interface::TokenAccount> =
            self.get_account_as(&destination_account);
        let balance = dest.map_or(0u64, |a| a.amount);

        let (ix, _) = make_mint_interchain_token_instruction(
            token_id,
            amount,
            minter,
            destination_account,
            token_program,
        );

        self.ctx
            .process_and_validate_instruction_chain(&[(&ix, &[Check::success()])]);

        // Check balance after minting
        let dest: anchor_spl::token_interface::TokenAccount = self
            .get_account_as(&destination_account)
            .expect("destination account should exist after minting");
        let new_balance = dest.amount;

        // Ensure balance increased by minted amount
        assert_eq!(
            new_balance,
            balance + amount,
            "destination account balance should increase by minted amount"
        );

        msg!(
            "Minted {} tokens of ID {} to account {}. Balance = {} -> {}",
            amount,
            hex::encode(token_id),
            destination_account,
            balance,
            new_balance,
        );
    }

    pub fn ensure_mint_test_interchain_token(
        &self,
        token_id: [u8; 32],
        amount: u64,
        destination_account: Pubkey,
    ) {
        let minter = self.operator;
        let token_program = spl_token_2022::ID;

        self.ensure_mint_interchain_token(
            token_id,
            amount,
            minter,
            destination_account,
            token_program,
        );
    }

    // TODO support token with fees
    pub fn ensure_outgoing_interchain_transfer(
        &self,
        token_id: [u8; 32],
        amount: u64,
        token_program: Pubkey,
        payer: Pubkey,
        authority: Pubkey,
        destination_chain: String,
        destination_address: Vec<u8>,
        gas_value: u64,
        caller_program_id: Option<Pubkey>,
        caller_pda_seeds: Option<Vec<Vec<u8>>>,
        data: Option<Vec<u8>>,
    ) {
        let token_mint = self.token_mint_for_id(token_id);

        // Get balance before transfer
        let authority_ata =
            get_associated_token_address_with_program_id(&authority, &token_mint, &token_program);
        let authority_ata_data: anchor_spl::token_interface::TokenAccount = self
            .get_account_as(&authority_ata)
            .expect("authority ata should exist");

        let balance_before = authority_ata_data.amount;

        // Transfer

        let (ix, _) = make_interchain_transfer_instruction(
            token_id,
            amount,
            token_program,
            payer,
            authority,
            destination_chain.clone(),
            destination_address,
            gas_value,
            caller_program_id,
            caller_pda_seeds,
            data,
        );

        self.ctx
            .process_and_validate_instruction_chain(&[(&ix, &[Check::success()])]);

        // Check balance after transfer

        let authority_ata_data: anchor_spl::token_interface::TokenAccount = self
            .get_account_as(&authority_ata)
            .expect("authority ata should exist");

        let balance_after = authority_ata_data.amount;

        assert_eq!(
            balance_after,
            balance_before - amount,
            "authority ata balance should decrease by transfer amount"
        );

        if let Some(caller_program_id) = caller_program_id {
            msg!(
                "Interchain transfer called by program {}",
                caller_program_id,
            );
        }

        msg!(
			"Interchain transfer of {} tokens of ID {} from {} to destination chain '{}' initiated. Balance: {} -> {}",
			amount,
			hex::encode(token_id),
			authority,
			destination_chain,
			balance_before,
			balance_after,
		);

        // TODO check event emission
    }

    pub fn ensure_outgoing_user_interchain_transfer(
        &self,
        token_id: [u8; 32],
        amount: u64,
        token_program: Pubkey,
        payer: Pubkey,
        authority: Pubkey,
        destination_chain: String,
        destination_address: Vec<u8>,
        gas_value: u64,
    ) {
        self.ensure_outgoing_interchain_transfer(
            token_id,
            amount,
            token_program,
            payer,
            authority,
            destination_chain,
            destination_address,
            gas_value,
            None,
            None,
            None,
        );
    }

    pub fn execute_gmp(&self, source_chain: &str, payload: GMPPayload) -> InstructionResult {
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

        let result = relayer_execute_with_checks(
            self.ctx(),
            &message,
            payload.encode(),
            Some(vec![vec![Check::success()]]),
        );

        assert!(result.is_ok(), "relayer discovery failed: {result:?}");

        result.unwrap()
    }

    pub fn execute_gmp_transfer(
        &self,
        token_id: [u8; 32],
        source_chain: &str,
        source_address: &str,
        destination_address: Pubkey,
        amount: u64,
        data: Option<Vec<u8>>,
    ) -> InstructionResult {
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
                .map_or(alloy_primitives::Bytes::new(), |d| d.clone().into()),
        };
        let transfer_payload_wrapped = GMPPayload::ReceiveFromHub(ReceiveFromHub {
            selector: ReceiveFromHub::MESSAGE_TYPE_ID_UINT,
            source_chain: source_chain.to_owned(),
            payload: GMPPayload::InterchainTransfer(transfer_payload)
                .encode()
                .into(),
        });

        self.execute_gmp(source_chain, transfer_payload_wrapped)
    }

    //
    // Memo Program
    //

    pub fn ensure_memo_program_initialized(&mut self) {
        let counter_pda = solana_axelar_memo::Counter::get_pda().0;
        if self.account_exists(&counter_pda) {
            return;
        }

        self.ctx.mollusk.add_program(
            &solana_axelar_memo::ID,
            "solana_axelar_memo",
            &solana_sdk_ids::bpf_loader_upgradeable::ID,
        );

        self.ctx.process_and_validate_instruction(
            &solana_axelar_memo::make_init_ix(self.payer),
            &[Check::success()],
        );

        let counter_account: solana_axelar_memo::Counter = self
            .get_account_as(&counter_pda)
            .expect("counter account should exist");

        assert_eq!(
            counter_account.counter, 0,
            "counter should have default value"
        );

        msg!("Memo program initialized.");
    }

    pub fn ensure_test_discoverable_program_initialized(&mut self) {
        let transaction_pda =
            relayer_discovery::find_transaction_pda(&solana_axelar_test_discoverable::ID).0;
        let its_transaction_pda =
            solana_axelar_its::utils::find_interchain_executable_transaction_pda(
                &solana_axelar_test_discoverable::ID,
            )
            .0;
        if self.account_exists(&transaction_pda) {
            return;
        }

        // Add memo program to the harness context
        self.ctx.mollusk.add_program(
            &solana_axelar_test_discoverable::ID,
            "solana_axelar_test_discoverable",
            &solana_sdk_ids::bpf_loader_upgradeable::ID,
        );

        self.ctx.process_and_validate_instruction(
            &solana_axelar_test_discoverable::make_init_ix(self.payer),
            &[Check::success()],
        );

        self.get_account(&transaction_pda)
            .expect("transaction account should exist");
        self.get_account(&its_transaction_pda)
            .expect("transaction account should exist");

        msg!("Test discoverable program initialized",);
    }
}
