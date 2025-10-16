use anchor_lang::prelude::Rent;
use anchor_lang::AnchorSerialize;
use anchor_lang::InstructionData;
use anchor_lang::{AccountDeserialize, ToAccountMetas};
use anchor_spl::{
    associated_token::spl_associated_token_account,
    token_2022::spl_token_2022::{
        self,
        extension::{
            transfer_fee::TransferFeeConfig, BaseStateWithExtensions, StateWithExtensions,
        },
    },
};
use axelar_solana_its_v2::{
    state::TokenManager,
    utils::{
        canonical_interchain_token_deploy_salt, canonical_interchain_token_id,
        interchain_token_id_internal,
    },
};
use mollusk_svm::{result::Check, Mollusk};
use mollusk_test_utils::get_event_authority_and_program_accounts;
use mpl_token_metadata::accounts::Metadata;
use solana_program::{instruction::Instruction, program_pack::Pack, system_program};
use solana_sdk::{
    account::Account, native_token::LAMPORTS_PER_SOL, pubkey::Pubkey, signature::Keypair,
    signer::Signer,
};
use spl_associated_token_account::get_associated_token_address_with_program_id;
use spl_token_2022::state::Account as Token2022Account;

#[path = "initialize.rs"]
mod initialize;

#[test]
fn test_register_canonical_token() {
    let program_id = axelar_solana_its_v2::id();
    let mollusk = initialize::initialize_mollusk();

    let payer = Pubkey::new_unique();
    let payer_account = Account::new(10 * LAMPORTS_PER_SOL, 0, &system_program::ID);

    let operator = Pubkey::new_unique();
    let operator_account = Account::new(1_000_000_000, 0, &system_program::ID);

    let chain_name = "solana".to_string();
    let its_hub_address = "0x123456789abcdef".to_string();

    // Initialize ITS service first
    let (
        its_root_pda,
        its_root_account,
        _user_roles_pda,
        _user_roles_account,
        _program_data,
        _program_data_account,
    ) = initialize::init_its_service(
        &mollusk,
        payer,
        &payer_account,
        payer,
        operator,
        &operator_account,
        chain_name.clone(),
        its_hub_address.clone(),
    );

    // Create a token mint (this would be an existing token we want to register as canonical)
    let mint_keypair = Keypair::new();
    let mint_pubkey = mint_keypair.pubkey();
    let mint_authority = Keypair::new();

    // Create a basic SPL token mint
    let mint_data = {
        let mut data = vec![0u8; spl_token_2022::state::Mint::LEN];
        let mint = spl_token_2022::state::Mint {
            mint_authority: Some(mint_authority.pubkey()).into(),
            supply: 1_000_000_000, // 1 billion tokens
            decimals: 9,
            is_initialized: true,
            freeze_authority: Some(mint_authority.pubkey()).into(),
        };
        spl_token_2022::state::Mint::pack(mint, &mut data).unwrap();
        data
    };

    let mint_account = Account {
        lamports: Rent::default().minimum_balance(spl_token_2022::state::Mint::LEN),
        data: mint_data,
        owner: spl_token_2022::ID,
        executable: false,
        rent_epoch: 0,
    };

    // Create metadata for the token
    let (metadata_account_pda, _metadata_bump) = Pubkey::find_program_address(
        &[
            b"metadata",
            mpl_token_metadata::programs::MPL_TOKEN_METADATA_ID.as_ref(),
            mint_pubkey.as_ref(),
        ],
        &mpl_token_metadata::programs::MPL_TOKEN_METADATA_ID,
    );

    let (event_authority, event_authority_account, program_account) =
        mollusk_test_utils::get_event_authority_and_program_accounts(&program_id);

    // Create metadata account data
    let metadata = Metadata {
        key: mpl_token_metadata::types::Key::MetadataV1,
        update_authority: mint_authority.pubkey(),
        mint: mint_pubkey,
        name: "Test Canonical Token".to_string(),
        symbol: "TCT".to_string(),
        uri: "https://example.com".to_string(),
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
    let metadata_account = Account {
        lamports: Rent::default().minimum_balance(metadata_data.len()),
        data: metadata_data,
        owner: mpl_token_metadata::programs::MPL_TOKEN_METADATA_ID,
        executable: false,
        rent_epoch: 0,
    };

    // Calculate the token ID and manager PDA
    let token_id = canonical_interchain_token_id(&mint_pubkey);
    let (token_manager_pda, _token_manager_bump) = Pubkey::find_program_address(
        &[
            axelar_solana_its_v2::seed_prefixes::TOKEN_MANAGER_SEED,
            its_root_pda.as_ref(),
            &token_id,
        ],
        &program_id,
    );

    // Calculate the token manager ATA
    let token_manager_ata = get_associated_token_address_with_program_id(
        &token_manager_pda,
        &mint_pubkey,
        &spl_token_2022::ID,
    );

    // Create the instruction
    let ix = Instruction {
        program_id,
        accounts: axelar_solana_its_v2::accounts::RegisterCanonicalInterchainToken {
            payer,
            metadata_account: metadata_account_pda,
            system_program: system_program::ID,
            its_root_pda,
            token_manager_pda,
            token_mint: mint_pubkey,
            token_manager_ata,
            token_program: spl_token_2022::ID,
            associated_token_program: spl_associated_token_account::ID,
            rent: solana_program::sysvar::rent::ID,
            // for event cpi
            event_authority,
            program: program_id,
        }
        .to_account_metas(None),
        data: axelar_solana_its_v2::instruction::RegisterCanonicalInterchainToken {}.data(),
    };

    // Set up accounts
    let accounts = vec![
        (payer, payer_account),
        (metadata_account_pda, metadata_account),
        (
            system_program::ID,
            Account {
                lamports: 1,
                data: vec![],
                owner: solana_sdk::native_loader::id(),
                executable: true,
                rent_epoch: 0,
            },
        ),
        (its_root_pda, its_root_account),
        (token_manager_pda, Account::new(0, 0, &system_program::ID)),
        (mint_pubkey, mint_account),
        (token_manager_ata, Account::new(0, 0, &system_program::ID)),
        mollusk_svm_programs_token::token2022::keyed_account(),
        mollusk_svm_programs_token::associated_token::keyed_account(),
        (
            solana_sdk::sysvar::rent::ID,
            Account {
                lamports: 1_000_000_000,
                data: axelar_solana_its_v2_test_fixtures::create_rent_sysvar_data(),
                owner: solana_sdk::sysvar::rent::ID,
                executable: false,
                rent_epoch: 0,
            },
        ),
        // For event CPI
        (event_authority, event_authority_account),
        (program_id, program_account),
    ];

    let result = mollusk.process_and_validate_instruction(&ix, &accounts, &[Check::success()]);

    assert!(
        result.program_result.is_ok(),
        "Register canonical token instruction should succeed: {:?}",
        result.program_result
    );

    // Verify token manager was created correctly
    let token_manager_account = result.get_account(&token_manager_pda).unwrap();
    let token_manager =
        TokenManager::try_deserialize(&mut token_manager_account.data.as_ref()).unwrap();

    let deploy_salt = canonical_interchain_token_deploy_salt(&mint_pubkey);
    let expected_token_id = interchain_token_id_internal(&deploy_salt);

    assert_eq!(token_manager.token_id, expected_token_id);
    assert_eq!(token_manager.token_address, mint_pubkey);
    assert_eq!(token_manager.associated_token_account, token_manager_ata);
    assert_eq!(token_manager.flow_slot.flow_limit, None);
    assert_eq!(token_manager.flow_slot.flow_in, 0);
    assert_eq!(token_manager.flow_slot.flow_out, 0);
    assert_eq!(token_manager.flow_slot.epoch, 0);
    assert_eq!(
        token_manager.ty,
        axelar_solana_its_v2::state::Type::LockUnlock
    ); // No fee extension

    // Verify token manager ATA was created
    let token_manager_ata_account = result.get_account(&token_manager_ata).unwrap();
    let token_manager_ata_data =
        StateWithExtensions::<Token2022Account>::unpack(&token_manager_ata_account.data).unwrap();

    assert_eq!(token_manager_ata_data.base.mint, mint_pubkey);
    assert_eq!(token_manager_ata_data.base.owner, token_manager_pda);
    assert_eq!(token_manager_ata_data.base.amount, 0);
}
