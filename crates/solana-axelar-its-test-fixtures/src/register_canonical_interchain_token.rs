use crate::create_rent_sysvar_data;
use anchor_lang::prelude::{borsh, Rent};
use anchor_lang::{InstructionData, ToAccountMetas};
use anchor_spl::{
    associated_token::{
        get_associated_token_address_with_program_id, spl_associated_token_account,
    },
    token_2022::spl_token_2022::{self},
};
use mollusk_svm::program::keyed_account_for_system_program;
use mollusk_svm::result::Check;
use mollusk_svm::{result::InstructionResult, Mollusk};
use mpl_token_metadata::accounts::Metadata;
use solana_axelar_its::state::TokenManager;
use solana_axelar_its::utils::canonical_interchain_token_id;
use solana_program::program_pack::Pack;
use solana_sdk::signature::Signer;
use solana_sdk::signer::keypair::Keypair;
use solana_sdk::{account::Account, instruction::Instruction, pubkey::Pubkey};

pub fn register_canonical_interchain_token_helper(
    mollusk: &Mollusk,
    mint_data: Vec<u8>,
    mint_keypair: &Keypair,
    mint_authority: &Keypair,
    payer: (Pubkey, Account),
    its_root: (Pubkey, Account),
    checks: Vec<Check>,
) -> InstructionResult {
    let program_id = solana_axelar_its::id();
    let mint_pubkey = mint_keypair.pubkey();

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
        name: "Test Canonical Token".to_owned(),
        symbol: "TCT".to_owned(),
        uri: "https://example.com".to_owned(),
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
    let metadata_account = Account {
        lamports: Rent::default().minimum_balance(metadata_data.len()),
        data: metadata_data,
        owner: mpl_token_metadata::programs::MPL_TOKEN_METADATA_ID,
        executable: false,
        rent_epoch: 0,
    };

    // Calculate the token ID and manager PDA
    let token_id = canonical_interchain_token_id(&mint_pubkey);
    let (token_manager_pda, _) = TokenManager::find_pda(token_id, its_root.0);

    // Calculate the token manager ATA
    let token_manager_ata = get_associated_token_address_with_program_id(
        &token_manager_pda,
        &mint_pubkey,
        &spl_token_2022::ID,
    );

    // Create the instruction
    let ix = Instruction {
        program_id,
        accounts: solana_axelar_its::accounts::RegisterCanonicalInterchainToken {
            payer: payer.0,
            metadata_account: metadata_account_pda,
            system_program: solana_sdk_ids::system_program::ID,
            its_root_pda: its_root.0,
            token_manager_pda,
            token_mint: mint_pubkey,
            token_manager_ata,
            token_program: spl_token_2022::ID,
            associated_token_program: spl_associated_token_account::program::ID,
            // for event cpi
            event_authority,
            program: program_id,
        }
        .to_account_metas(None),
        data: solana_axelar_its::instruction::RegisterCanonicalInterchainToken {}.data(),
    };

    // Set up accounts
    let accounts = vec![
        (payer.0, payer.1),
        (metadata_account_pda, metadata_account),
        keyed_account_for_system_program(),
        (its_root.0, its_root.1),
        (
            token_manager_pda,
            Account::new(0, 0, &solana_sdk_ids::system_program::ID),
        ),
        (mint_pubkey, mint_account),
        (
            token_manager_ata,
            Account::new(0, 0, &solana_sdk_ids::system_program::ID),
        ),
        mollusk_svm_programs_token::token2022::keyed_account(),
        mollusk_svm_programs_token::associated_token::keyed_account(),
        (
            solana_sdk::sysvar::rent::ID,
            Account {
                lamports: 1_000_000_000,
                data: create_rent_sysvar_data(),
                owner: solana_sdk::sysvar::rent::ID,
                executable: false,
                rent_epoch: 0,
            },
        ),
        // For event CPI
        (event_authority, event_authority_account),
        (program_id, program_account),
    ];

    mollusk.process_and_validate_instruction(&ix, &accounts, &checks)
}
