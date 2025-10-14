use anchor_lang::{InstructionData, ToAccountMetas};
use anchor_spl::{
    associated_token::get_associated_token_address_with_program_id,
    token_2022::spl_token_2022::{self},
};
use axelar_solana_gas_service_v2::state::Treasury;
use axelar_solana_gateway_v2::seed_prefixes::CALL_CONTRACT_SIGNING_SEED;
use axelar_solana_its_v2::{
    seed_prefixes::{INTERCHAIN_TOKEN_SEED, TOKEN_MANAGER_SEED},
    utils::interchain_token_id,
};
use mollusk_svm::{result::InstructionResult, Mollusk};
use mollusk_svm_programs_token;
use mollusk_test_utils::get_event_authority_and_program_accounts;

use solana_sdk::{
    account::Account, instruction::Instruction, native_token::LAMPORTS_PER_SOL, pubkey::Pubkey,
    system_program,
};

pub struct DeployInterchainTokenContext {
    mollusk: Mollusk,
    its_root_pda: Pubkey,
    its_root_account: Account,
    deployer: Pubkey,
    deployer_account: Account,
    program_id: Pubkey,
    payer: Pubkey,
    payer_account: Account,
    minter: Option<Pubkey>,
    minter_roles_pda: Option<Pubkey>,
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
        minter: Option<Pubkey>,
        minter_roles_pda: Option<Pubkey>,
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
            minter,
            minter_roles_pda,
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
        salt,
        name: name.clone(),
        symbol: symbol.clone(),
        decimals,
        initial_supply,
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
            minter: ctx.minter,
            minter_roles_pda: ctx.minter_roles_pda,
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
        // Minter accounts - use program_id as placeholder if None
        (
            ctx.minter.unwrap_or(ctx.program_id),
            Account::new(1_000_000_000, 0, &system_program::ID),
        ),
        (
            ctx.minter_roles_pda.unwrap_or(ctx.program_id),
            Account::new(0, 0, &system_program::ID),
        ),
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

pub fn deploy_remote_interchain_token_helper(
    salt: [u8; 32],
    destination_chain: String,
    gas_value: u64,
    // ctx
    result: InstructionResult,
    mollusk: &Mollusk,
    program_id: Pubkey,
    payer: Pubkey,
    deployer: Pubkey,
    token_mint_pda: Pubkey,
    metadata_account: Pubkey,
    token_manager_pda: Pubkey,
    its_root_pda: Pubkey,
    treasury_pda: Account,
    gateway_root_pda_account: Account,
) -> InstructionResult {
    let (gateway_root_pda, _) = Pubkey::find_program_address(
        &[axelar_solana_gateway_v2::seed_prefixes::GATEWAY_SEED],
        &axelar_solana_gateway_v2::ID,
    );

    let (gas_treasury, _) =
        Pubkey::find_program_address(&[Treasury::SEED_PREFIX], &axelar_solana_gas_service_v2::ID);

    let (call_contract_signing_pda, signing_pda_bump) =
        Pubkey::find_program_address(&[CALL_CONTRACT_SIGNING_SEED], &program_id);

    let (gateway_event_authority, _) =
        Pubkey::find_program_address(&[b"__event_authority"], &axelar_solana_gateway_v2::ID);

    let (gas_event_authority, _) =
        Pubkey::find_program_address(&[b"__event_authority"], &axelar_solana_gas_service_v2::ID);

    let (its_event_authority, its_event_authority_account, its_program_account) =
        get_event_authority_and_program_accounts(&program_id);

    let ix = Instruction {
        program_id,
        accounts: axelar_solana_its_v2::accounts::DeployRemoteInterchainToken {
            payer,
            deployer,
            token_mint: token_mint_pda,
            metadata_account,
            token_manager_pda,
            gateway_root_pda,
            axelar_gateway_program: axelar_solana_gateway_v2::ID,
            gas_treasury,
            gas_service: axelar_solana_gas_service_v2::ID,
            system_program: system_program::ID,
            its_root_pda,
            call_contract_signing_pda,
            its_program: program_id,
            gateway_event_authority,
            gas_event_authority,
            event_authority: its_event_authority,
            program: program_id,
        }
        .to_account_metas(None),
        data: axelar_solana_its_v2::instruction::DeployRemoteInterchainToken {
            salt,
            destination_chain: destination_chain.clone(),
            gas_value,
            signing_pda_bump,
        }
        .data(),
    };

    // Get the updated accounts from the first instruction result
    let updated_mollusk = mollusk;
    let updated_payer_account = result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == payer)
        .map(|(_, account)| account.clone())
        .unwrap_or_else(|| Account::new(9 * LAMPORTS_PER_SOL, 0, &system_program::ID));

    let updated_its_root_account = result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == its_root_pda)
        .unwrap()
        .1
        .clone();

    let updated_token_mint_account = result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == token_mint_pda)
        .unwrap()
        .1
        .clone();

    let updated_metadata_account = result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == metadata_account)
        .unwrap()
        .1
        .clone();

    let updated_token_manager_account = result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == token_manager_pda)
        .unwrap()
        .1
        .clone();

    // Accounts for the deploy remote instruction
    let accounts = vec![
        (payer, updated_payer_account),
        (
            deployer,
            Account::new(10 * LAMPORTS_PER_SOL, 0, &system_program::ID),
        ),
        (token_mint_pda, updated_token_mint_account),
        (metadata_account, updated_metadata_account),
        (token_manager_pda, updated_token_manager_account),
        (gateway_root_pda, gateway_root_pda_account.clone()),
        (
            axelar_solana_gateway_v2::ID,
            Account {
                lamports: 1,
                data: vec![],
                owner: solana_sdk::bpf_loader_upgradeable::ID,
                executable: true,
                rent_epoch: 0,
            },
        ),
        (gas_treasury, treasury_pda.clone()),
        (
            axelar_solana_gas_service_v2::ID,
            Account {
                lamports: 1,
                data: vec![],
                owner: solana_sdk::bpf_loader_upgradeable::ID,
                executable: true,
                rent_epoch: 0,
            },
        ),
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
        (its_root_pda, updated_its_root_account),
        (call_contract_signing_pda, Account::new(0, 0, &program_id)),
        (
            program_id,
            Account {
                lamports: 1,
                data: vec![],
                owner: solana_sdk::bpf_loader_upgradeable::ID,
                executable: true,
                rent_epoch: 0,
            },
        ),
        (
            gateway_event_authority,
            Account::new(0, 0, &system_program::ID),
        ),
        (gas_event_authority, Account::new(0, 0, &system_program::ID)),
        // For event cpi
        (its_event_authority, its_event_authority_account),
        (program_id, its_program_account),
    ];

    updated_mollusk.process_instruction(&ix, &accounts)
}
