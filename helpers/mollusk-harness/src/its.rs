use std::collections::HashMap;

use anchor_lang::prelude::borsh;
use anchor_lang::{prelude::AccountMeta, InstructionData, ToAccountMetas};
use anchor_spl::{
    associated_token::{self, get_associated_token_address_with_program_id},
    token_2022::spl_token_2022,
};
use mollusk_svm::{
    result::{Check, InstructionResult},
    Mollusk, MolluskContext,
};
use mollusk_test_utils::get_event_authority_and_program_accounts;
use rand::Rng;
use solana_axelar_gateway::Message as CrossChainMessage;
use solana_axelar_its::{
    encoding,
    instructions::{
        execute_interchain_transfer_extra_accounts, make_deploy_interchain_token_instruction,
        make_interchain_transfer_instruction, make_mint_interchain_token_instruction,
        make_register_canonical_token_instruction, make_set_trusted_chain_instruction,
    },
    InterchainTokenService,
};
use solana_sdk::{
    account::Account, instruction::Instruction, native_token::LAMPORTS_PER_SOL, pubkey::Pubkey,
};

use crate::gateway::{GatewayHarnessInfo, GatewaySetup};
use crate::{msg, TestHarness};

/// Creates a Mollusk instance with ITS, gateway, and all dependencies loaded.
pub fn initialize_its_mollusk() -> Mollusk {
    std::env::set_var("SBF_OUT_DIR", "../../target/deploy");
    let mut mollusk = Mollusk::new(&solana_axelar_its::ID, "solana_axelar_its");

    // Operators
    mollusk.add_program(&solana_axelar_operators::ID, "solana_axelar_operators");

    // Gas Service
    mollusk.add_program(&solana_axelar_gas_service::ID, "solana_axelar_gas_service");

    // Gateway
    mollusk.add_program(&solana_axelar_gateway::ID, "solana_axelar_gateway");

    // Token Programs
    mollusk.add_program_with_loader_and_elf(
        &spl_token::ID,
        &solana_sdk_ids::bpf_loader_upgradeable::ID,
        mollusk_svm_programs_token::token::ELF,
    );
    mollusk.add_program_with_loader_and_elf(
        &spl_token_2022::ID,
        &solana_sdk_ids::bpf_loader_upgradeable::ID,
        mollusk_svm_programs_token::token2022::ELF,
    );
    mollusk.add_program_with_loader_and_elf(
        &anchor_spl::associated_token::ID,
        &solana_sdk_ids::bpf_loader_upgradeable::ID,
        mollusk_svm_programs_token::associated_token::ELF,
    );
    mollusk.add_program(
        &mpl_token_metadata::programs::MPL_TOKEN_METADATA_ID,
        "../../programs/solana-axelar-its/tests/mpl_token_metadata",
    );

    mollusk
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

impl GatewaySetup for ItsTestHarness {
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

impl Default for ItsTestHarness {
    /// Create a default ITS test harness without initializing ITS.
    /// Useful for testing initialization itself.
    fn default() -> Self {
        let mollusk = initialize_its_mollusk();

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

    pub fn ensure_its_initialized(&mut self) {
        self.ensure_gateway_initialized();

        let its_root_pda = InterchainTokenService::find_pda().0;
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
    /// Reads the actual token address from the token manager account,
    /// which works for both interchain tokens (PDA mint) and canonical
    /// tokens (external mint). Falls back to deriving the PDA if the
    /// token manager doesn't exist yet.
    pub fn token_mint_for_id(&self, token_id: [u8; 32]) -> Pubkey {
        let token_manager_pda =
            solana_axelar_its::TokenManager::find_pda(token_id, self.its_root).0;
        if let Some(tm) = self.get_account_as::<solana_axelar_its::TokenManager>(&token_manager_pda)
        {
            tm.token_address
        } else {
            solana_axelar_its::TokenManager::find_token_mint(token_id, self.its_root).0
        }
    }

    /// Returns the expected `destination_token_authority` for a given destination.
    ///
    /// If the destination account is owned by a BPF loader (i.e. it is a deployed
    /// program), the authority is the PDA `[ITS_TOKEN_AUTHORITY_SEED]` derived from
    /// the destination program. Otherwise it is the destination address itself.
    pub fn expected_destination_token_authority(&self, destination: &Pubkey) -> Pubkey {
        let is_program = self.get_account(destination).is_some_and(|acc| {
            acc.owner == solana_sdk_ids::bpf_loader::ID
                || acc.owner == solana_sdk_ids::bpf_loader_deprecated::ID
                || acc.owner == solana_sdk_ids::bpf_loader_upgradeable::ID
                || acc.owner == solana_sdk_ids::loader_v4::ID
        });

        if is_program {
            solana_axelar_its::instructions::destination_token_authority_pda(destination)
        } else {
            *destination
        }
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
        let should_be_closed = old_its_roles.roles == solana_axelar_its::roles::OPERATOR;

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

    pub fn execute_hub_message(
        &self,
        token_id: [u8; 32],
        source_chain: &str,
        payload: encoding::HubMessage,
        extra_accounts: Vec<AccountMeta>,
    ) -> InstructionResult {
        self.execute_hub_message_with_checks(
            token_id,
            source_chain,
            payload,
            extra_accounts,
            &[Check::success()],
        )
    }

    /// Like `execute_hub_message` but accepts custom checks instead of asserting success.
    pub fn execute_hub_message_with_checks(
        &self,
        token_id: [u8; 32],
        source_chain: &str,
        payload: encoding::HubMessage,
        extra_accounts: Vec<AccountMeta>,
        checks: &[Check],
    ) -> InstructionResult {
        let encoded_payload = borsh::to_vec(&payload).expect("payload should serialize");
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

        let message = CrossChainMessage {
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
        let token_mint = self.token_mint_for_id(token_id);
        let token_manager_ata = get_associated_token_address_with_program_id(
            &token_manager_pda,
            &token_mint,
            &spl_token_2022::ID,
        );

        let executable = solana_axelar_its::accounts::AxelarExecuteAccounts {
            incoming_message_pda,
            signing_pda: solana_axelar_gateway::ValidateMessageSigner::create_pda(
                &message.command_id(),
                incoming_message.signing_pda_bump,
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
            system_program: solana_sdk_ids::system_program::ID,
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
            .process_and_validate_instruction_chain(&[(&ix, checks)])
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

        let transfer_payload = encoding::InterchainTransfer {
            token_id,
            source_address: source_address.as_bytes().to_vec(),
            destination_address: destination_address.to_bytes().to_vec(),
            amount,
            data: data.clone().map(|(d, _)| d),
        };

        let token_mint = self.token_mint_for_id(token_id);

        let destination_token_authority =
            self.expected_destination_token_authority(&destination_address);

        let destination_ata = get_associated_token_address_with_program_id(
            &destination_token_authority,
            &token_mint,
            &spl_token_2022::ID,
        );

        let mut extra_accounts = execute_interchain_transfer_extra_accounts(
            destination_address,
            destination_token_authority,
            destination_ata,
            Some(has_data),
        );
        if let Some((_, data_accounts)) = data {
            extra_accounts.extend(data_accounts);
        }

        let transfer_payload_wrapped = encoding::HubMessage::ReceiveFromHub {
            source_chain: source_chain.to_owned(),
            message: encoding::Message::InterchainTransfer(transfer_payload),
        };

        self.execute_hub_message(
            token_id,
            source_chain,
            transfer_payload_wrapped,
            extra_accounts,
        )
    }

    /// Like `execute_gmp_transfer` but allows overriding the destination token
    /// authority and result checks. Useful for testing invalid authority scenarios.
    pub fn execute_gmp_transfer_with_authority(
        &self,
        token_id: [u8; 32],
        source_chain: &str,
        source_address: &str,
        destination_address: Pubkey,
        amount: u64,
        data: Option<(Vec<u8>, Vec<AccountMeta>)>,
        destination_token_authority: Pubkey,
        checks: &[Check],
    ) -> InstructionResult {
        let has_data = data.is_some();

        let transfer_payload = encoding::InterchainTransfer {
            token_id,
            source_address: source_address.as_bytes().to_vec(),
            destination_address: destination_address.to_bytes().to_vec(),
            amount,
            data: data.clone().map(|(d, _)| d),
        };

        let token_mint = self.token_mint_for_id(token_id);

        let destination_ata = get_associated_token_address_with_program_id(
            &destination_token_authority,
            &token_mint,
            &spl_token_2022::ID,
        );

        let mut extra_accounts = execute_interchain_transfer_extra_accounts(
            destination_address,
            destination_token_authority,
            destination_ata,
            Some(has_data),
        );
        if let Some((_, data_accounts)) = data {
            extra_accounts.extend(data_accounts);
        }

        let transfer_payload_wrapped = encoding::HubMessage::ReceiveFromHub {
            source_chain: source_chain.to_owned(),
            message: encoding::Message::InterchainTransfer(transfer_payload),
        };

        self.execute_hub_message_with_checks(
            token_id,
            source_chain,
            transfer_payload_wrapped,
            extra_accounts,
            checks,
        )
    }

    /// Execute a GMP LinkToken message. Takes an explicit `token_mint` because
    /// link-token operates on an existing SPL mint (not a PDA-derived one).
    pub fn execute_gmp_link_token(
        &self,
        token_id: [u8; 32],
        source_chain: &str,
        token_mint: Pubkey,
        payload: encoding::LinkToken,
        extra_accounts: Vec<AccountMeta>,
    ) -> InstructionResult {
        self.execute_gmp_link_token_with_checks(
            token_id,
            source_chain,
            token_mint,
            payload,
            extra_accounts,
            &[Check::success()],
        )
    }

    /// Like `execute_gmp_link_token` but accepts custom checks.
    pub fn execute_gmp_link_token_with_checks(
        &self,
        token_id: [u8; 32],
        source_chain: &str,
        token_mint: Pubkey,
        payload: encoding::LinkToken,
        extra_accounts: Vec<AccountMeta>,
        checks: &[Check],
    ) -> InstructionResult {
        let hub_message = encoding::HubMessage::ReceiveFromHub {
            source_chain: source_chain.to_owned(),
            message: encoding::Message::LinkToken(payload),
        };

        let encoded_payload = borsh::to_vec(&hub_message).expect("payload should serialize");
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

        let message = CrossChainMessage {
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
        let token_manager_ata = get_associated_token_address_with_program_id(
            &token_manager_pda,
            &token_mint,
            &spl_token_2022::ID,
        );

        let executable = solana_axelar_its::accounts::AxelarExecuteAccounts {
            incoming_message_pda,
            signing_pda: solana_axelar_gateway::ValidateMessageSigner::create_pda(
                &message.command_id(),
                incoming_message.signing_pda_bump,
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
            system_program: solana_sdk_ids::system_program::ID,
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
            .process_and_validate_instruction_chain(&[(&ix, checks)])
    }

    //
    // Memo Program
    //

    pub fn ensure_memo_program_initialized(&mut self) {
        let counter_pda = solana_axelar_memo::Counter::find_pda().0;
        if self.account_exists(&counter_pda) {
            return;
        }

        self.ctx
            .mollusk
            .add_program(&solana_axelar_memo::ID, "solana_axelar_memo");

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
}
