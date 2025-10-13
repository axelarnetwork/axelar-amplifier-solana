use anchor_lang::{AccountDeserialize, AnchorDeserialize, InstructionData, ToAccountMetas};
use anchor_spl::{
    associated_token::get_associated_token_address_with_program_id,
    token::spl_token,
    token_2022::spl_token_2022::{self, extension::StateWithExtensions},
};
use axelar_solana_its_v2::{
    instructions::DeployInterchainTokenData,
    seed_prefixes::{INTERCHAIN_TOKEN_SEED, TOKEN_MANAGER_SEED},
    state::TokenManager,
    utils::{interchain_token_deployer_salt, interchain_token_id, interchain_token_id_internal},
};
use mollusk_svm::{result::InstructionResult, Mollusk};
use mollusk_svm_programs_token;
use mollusk_test_utils::{get_event_authority_and_program_accounts, setup_mollusk};
use solana_program::program_pack::Pack;
use solana_sdk::{
    account::Account, instruction::Instruction, native_token::LAMPORTS_PER_SOL, pubkey::Pubkey,
    system_program,
};
use spl_token_2022::state::Account as Token2022Account;

pub struct DeployInterchainTokenContext {
    mollusk: Mollusk,
    its_root_pda: Pubkey,
    its_root_account: Account,
    deployer: Pubkey,
    deployer_account: Account,
    program_id: Pubkey,
    payer: Pubkey,
    payer_account: Account,
}

impl DeployInterchainTokenContext {
    pub fn new(
        mollusk: Mollusk,
        its_root_pda: Pubkey,
        its_root_account: Account,
        deployer: Pubkey,
        deployer_account: Account,
        program_id: Pubkey,
        payer: Pubkey,
        payer_account: Account,
    ) -> Self {
        Self {
            mollusk,
            its_root_pda,
            its_root_account,
            deployer,
            deployer_account,
            program_id,
            payer,
            payer_account,
        }
    }
}

pub fn create_rent_sysvar_data() -> Vec<u8> {
    use solana_sdk::rent::Rent;

    let rent = Rent::default();
    bincode::serialize(&rent).unwrap()
}

pub fn create_sysvar_instructions_data() -> Vec<u8> {
    use solana_sdk::sysvar::instructions::{construct_instructions_data, BorrowedInstruction};

    let instructions: &[BorrowedInstruction] = &[];
    construct_instructions_data(instructions)
}

pub fn deploy_interchain_token_helper(
    salt: [u8; 32],
    name: String,
    symbol: String,
    decimals: u8,
    initial_supply: u64,
    minter: Option<Pubkey>,
    ctx: DeployInterchainTokenContext,
) -> (
    InstructionResult,
    Pubkey,
    Pubkey,
    Pubkey,
    Pubkey,
    Pubkey,
    Mollusk,
) {
    let deploy_params = DeployInterchainTokenData {
        salt,
        name: name.clone(),
        symbol: symbol.clone(),
        decimals,
        initial_supply,
        minter,
    };

    let token_id = interchain_token_id(&ctx.deployer, &salt);

    let (token_manager_pda, _) = Pubkey::find_program_address(
        &[TOKEN_MANAGER_SEED, ctx.its_root_pda.as_ref(), &token_id],
        &ctx.program_id,
    );

    let (token_mint_pda, _) = Pubkey::find_program_address(
        &[INTERCHAIN_TOKEN_SEED, ctx.its_root_pda.as_ref(), &token_id],
        &ctx.program_id,
    );

    let token_manager_ata = get_associated_token_address_with_program_id(
        &token_manager_pda,
        &token_mint_pda,
        &spl_token_2022::ID,
    );

    let deployer_ata = get_associated_token_address_with_program_id(
        &ctx.deployer,
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
        get_event_authority_and_program_accounts(&ctx.program_id);

    let ix_data = axelar_solana_its_v2::instruction::DeployInterchainToken {
        params: deploy_params,
    }
    .data();

    let ix = Instruction {
        program_id: ctx.program_id,
        accounts: axelar_solana_its_v2::accounts::DeployInterchainToken {
            payer: ctx.payer,
            deployer: ctx.deployer,
            system_program: system_program::ID,
            its_root_pda: ctx.its_root_pda,
            token_manager_pda,
            token_mint: token_mint_pda,
            token_manager_ata,
            token_program: spl_token_2022::ID,
            associated_token_program: anchor_spl::associated_token::ID,
            rent: solana_sdk::sysvar::rent::ID,
            sysvar_instructions: solana_sdk::sysvar::instructions::ID,
            mpl_token_metadata_program: mpl_token_metadata::programs::MPL_TOKEN_METADATA_ID,
            mpl_token_metadata_account: metadata_account,
            deployer_ata,
            event_authority,
            program: ctx.program_id,
        }
        .to_account_metas(None),
        data: ix_data,
    };

    let accounts = vec![
        (ctx.payer, ctx.payer_account),
        (ctx.deployer, ctx.deployer_account),
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
        (ctx.its_root_pda, ctx.its_root_account),
        (token_manager_pda, Account::new(0, 0, &system_program::ID)),
        (token_mint_pda, Account::new(0, 0, &system_program::ID)),
        (token_manager_ata, Account::new(0, 0, &system_program::ID)),
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
        (metadata_account, Account::new(0, 0, &system_program::ID)),
        (deployer_ata, Account::new(0, 0, &system_program::ID)),
        // For event CPI
        (event_authority, event_authority_account),
        (ctx.program_id, program_account),
    ];

    (
        ctx.mollusk.process_instruction(&ix, &accounts),
        token_manager_pda,
        token_mint_pda,
        token_manager_ata,
        deployer_ata,
        metadata_account,
        ctx.mollusk,
    )
}
