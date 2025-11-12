use crate::{create_rent_sysvar_data, create_sysvar_instructions_data};
use anchor_lang::{InstructionData, ToAccountMetas};
use anchor_spl::{
    associated_token::get_associated_token_address_with_program_id,
    token_2022::spl_token_2022::{self},
};
use mollusk_svm::{program::keyed_account_for_system_program, result::Check};
use mollusk_svm::{result::InstructionResult, Mollusk};
use mollusk_test_utils::get_event_authority_and_program_accounts;
use solana_axelar_its::{
    seed_prefixes::INTERCHAIN_TOKEN_SEED, state::TokenManager, utils::interchain_token_id,
};
use solana_sdk::{account::Account, instruction::Instruction, pubkey::Pubkey};

pub struct DeployInterchainTokenContext {
    mollusk: Mollusk,
    its_root: (Pubkey, Account),
    deployer: (Pubkey, Account),
    payer: (Pubkey, Account),
    minter: Option<Pubkey>,
    minter_roles_pda: Option<Pubkey>,
}

impl DeployInterchainTokenContext {
    pub fn new(
        mollusk: Mollusk,
        its_root: (Pubkey, Account),
        deployer: (Pubkey, Account),
        payer: (Pubkey, Account),
        minter: Option<Pubkey>,
        minter_roles_pda: Option<Pubkey>,
    ) -> Self {
        Self {
            mollusk,
            its_root,
            deployer,
            payer,
            minter,
            minter_roles_pda,
        }
    }
}

pub fn deploy_interchain_token_helper(
    ctx: DeployInterchainTokenContext,
    salt: [u8; 32],
    name: String,
    symbol: String,
    decimals: u8,
    initial_supply: u64,
    checks: Vec<Check>,
) -> (
    InstructionResult,
    Pubkey,
    Pubkey,
    Pubkey,
    Pubkey,
    Pubkey,
    Mollusk,
) {
    let program_id = solana_axelar_its::id();
    let token_id = interchain_token_id(&ctx.deployer.0, &salt);
    let (token_manager_pda, _) = TokenManager::find_pda(token_id, ctx.its_root.0);

    let (token_mint_pda, _) = Pubkey::find_program_address(
        &[INTERCHAIN_TOKEN_SEED, ctx.its_root.0.as_ref(), &token_id],
        &program_id,
    );

    let token_manager_ata = get_associated_token_address_with_program_id(
        &token_manager_pda,
        &token_mint_pda,
        &spl_token_2022::ID,
    );

    let deployer_ata = get_associated_token_address_with_program_id(
        &ctx.deployer.0,
        &token_mint_pda,
        &spl_token_2022::ID,
    );

    let (metadata_account, _) = Pubkey::find_program_address(
        &[
            b"metadata",
            mpl_token_metadata::programs::MPL_TOKEN_METADATA_ID.as_ref(),
            token_mint_pda.as_ref(),
        ],
        &mpl_token_metadata::programs::MPL_TOKEN_METADATA_ID,
    );

    let (event_authority, event_authority_account, program_account) =
        get_event_authority_and_program_accounts(&program_id);

    let ix_data = solana_axelar_its::instruction::DeployInterchainToken {
        salt,
        name: name.clone(),
        symbol: symbol.clone(),
        decimals,
        initial_supply,
    }
    .data();

    let ix = Instruction {
        program_id,
        accounts: solana_axelar_its::accounts::DeployInterchainToken {
            payer: ctx.payer.0,
            deployer: ctx.deployer.0,
            system_program: solana_sdk::system_program::ID,
            its_root_pda: ctx.its_root.0,
            token_manager_pda,
            token_mint: token_mint_pda,
            token_manager_ata,
            token_program: spl_token_2022::ID,
            associated_token_program: anchor_spl::associated_token::ID,
            sysvar_instructions: solana_sdk::sysvar::instructions::ID,
            mpl_token_metadata_program: mpl_token_metadata::programs::MPL_TOKEN_METADATA_ID,
            mpl_token_metadata_account: metadata_account,
            deployer_ata,
            minter: ctx.minter,
            minter_roles_pda: ctx.minter_roles_pda,
            event_authority,
            program: program_id,
        }
        .to_account_metas(None),
        data: ix_data,
    };

    let accounts = vec![
        (ctx.payer.0, ctx.payer.1),
        (ctx.deployer.0, ctx.deployer.1),
        keyed_account_for_system_program(),
        (ctx.its_root.0, ctx.its_root.1),
        (
            token_manager_pda,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
        (
            token_mint_pda,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
        (
            token_manager_ata,
            Account::new(0, 0, &solana_sdk::system_program::ID),
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
        // Instructions sysvar
        (
            solana_sdk::sysvar::instructions::ID,
            Account {
                lamports: 1_000_000_000,
                data: create_sysvar_instructions_data(),
                owner: solana_program::sysvar::id(),
                executable: false,
                rent_epoch: 0,
            },
        ),
        // MPL Token Metadata program
        (
            mpl_token_metadata::programs::MPL_TOKEN_METADATA_ID,
            Account {
                lamports: 1,
                data: vec![],
                owner: solana_sdk::bpf_loader_upgradeable::id(),
                executable: true,
                rent_epoch: 0,
            },
        ),
        (
            metadata_account,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
        (
            deployer_ata,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
        // Minter accounts - use program_id as placeholder if None
        (
            ctx.minter.unwrap_or(program_id),
            Account::new(1_000_000_000, 0, &solana_sdk::system_program::ID),
        ),
        (
            ctx.minter_roles_pda.unwrap_or(program_id),
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
        // For event CPI
        (event_authority, event_authority_account),
        (program_id, program_account),
    ];

    (
        ctx.mollusk
            .process_and_validate_instruction(&ix, &accounts, &checks),
        token_manager_pda,
        token_mint_pda,
        token_manager_ata,
        deployer_ata,
        metadata_account,
        ctx.mollusk,
    )
}
