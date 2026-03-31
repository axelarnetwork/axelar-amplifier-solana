#![allow(clippy::indexing_slicing)]
#![allow(clippy::too_many_arguments)]

pub mod gateway;
pub mod its;

// Re-exports for convenience
pub use gateway::{GatewayHarnessInfo, GatewayTestHarness};
pub use its::ItsTestHarness;

use std::collections::HashMap;

use anchor_lang::prelude::{borsh, bpf_loader_upgradeable};
use anchor_spl::{
    associated_token::{self, get_associated_token_address_with_program_id},
    token_2022::spl_token_2022,
};
use mollusk_svm::{result::Check, MolluskContext};
use mollusk_test_utils::create_program_data_account;
use mollusk_test_utils::system_account_with_lamports;
use solana_sdk::{account::Account, native_token::LAMPORTS_PER_SOL, pubkey::Pubkey};

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

pub(crate) use msg;

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
        let program_data = bpf_loader_upgradeable::get_program_data_address(program);
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

        let sysvar_account = Account {
            lamports: 1_000_000_000,
            data: construct_instructions_data(&[] as &[BorrowedInstruction]),
            owner: solana_sdk_ids::sysvar::ID,
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

        let metadata_data = borsh::to_vec(&metadata).unwrap();
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
