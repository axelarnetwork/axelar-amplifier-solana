use anchor_lang::prelude::Rent;
use anchor_lang::{AccountDeserialize, AnchorSerialize};
use anchor_lang::{Discriminator, InstructionData, Space, ToAccountMetas};
use anchor_spl::token::spl_token;
use anchor_spl::{
    associated_token::{
        get_associated_token_address_with_program_id, spl_associated_token_account,
    },
    token_2022::spl_token_2022::{self},
};
use axelar_solana_gas_service_v2::state::Treasury;
use axelar_solana_gateway_v2::seed_prefixes::CALL_CONTRACT_SIGNING_SEED;
use axelar_solana_its_v2::state::{InterchainTokenService, Roles, UserRoles};
use axelar_solana_its_v2::{
    seed_prefixes::{INTERCHAIN_TOKEN_SEED, TOKEN_MANAGER_SEED},
    utils::{canonical_interchain_token_id, interchain_token_id},
};
use axelar_solana_operators::{OperatorAccount, OperatorRegistry};
use mollusk_svm::program::keyed_account_for_system_program;
use mollusk_svm::result::Check;
use mollusk_svm::{result::InstructionResult, Mollusk};
use mollusk_svm_programs_token;
use mollusk_test_utils::{
    create_program_data_account, get_event_authority_and_program_accounts, setup_mollusk,
};
use mpl_token_metadata::accounts::Metadata;
use solana_program::program_pack::Pack;
use solana_sdk::signature::Signer;
use solana_sdk::signer::keypair::Keypair;

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

pub fn system_account_tuple() -> (Pubkey, Account) {
    (
        system_program::ID,
        Account {
            lamports: 1,
            data: vec![],
            owner: solana_sdk::native_loader::id(),
            executable: true,
            rent_epoch: 0,
        },
    )
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
        system_account_tuple(),
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

pub struct DeployRemoteInterchainTokenContext {
    result: InstructionResult,
    mollusk: Mollusk,
    program_id: Pubkey,
    payer: Pubkey,
    deployer: Pubkey,
    token_mint_pda: Pubkey,
    metadata_account: Pubkey,
    token_manager_pda: Pubkey,
    its_root_pda: Pubkey,
    treasury_pda: Account,
    gateway_root_pda_account: Account,
    // Optional minter fields
    minter: Option<Pubkey>,
    deploy_approval_pda: Option<Pubkey>,
    deploy_approval_pda_account: Option<Account>,
    minter_roles: Option<Pubkey>,
    minter_roles_account: Option<Account>,
}

impl DeployRemoteInterchainTokenContext {
    pub fn new(
        result: InstructionResult,
        mollusk: Mollusk,
        program_id: Pubkey,
        payer: Pubkey,
        deployer: Pubkey,
        token_mint_pda: Pubkey,
        metadata_account: Pubkey,
        token_manager_pda: Pubkey,
        its_root_pda: Pubkey,
        treasury_pda: Account,
        gateway_root_pda_account: Account,
    ) -> Self {
        Self {
            result,
            mollusk,
            program_id,
            payer,
            deployer,
            token_mint_pda,
            metadata_account,
            token_manager_pda,
            its_root_pda,
            treasury_pda,
            gateway_root_pda_account,
            minter: None,
            deploy_approval_pda: None,
            deploy_approval_pda_account: None,
            minter_roles: None,
            minter_roles_account: None,
        }
    }

    pub fn new_with_minter(
        result: InstructionResult,
        mollusk: Mollusk,
        program_id: Pubkey,
        payer: Pubkey,
        deployer: Pubkey,
        token_mint_pda: Pubkey,
        metadata_account: Pubkey,
        token_manager_pda: Pubkey,
        its_root_pda: Pubkey,
        treasury_pda: Account,
        gateway_root_pda_account: Account,
        minter: Pubkey,
        deploy_approval_pda: Pubkey,
        deploy_approval_pda_account: Account,
        minter_roles: Pubkey,
        minter_roles_account: Account,
    ) -> Self {
        Self {
            result,
            mollusk,
            program_id,
            payer,
            deployer,
            token_mint_pda,
            metadata_account,
            token_manager_pda,
            its_root_pda,
            treasury_pda,
            gateway_root_pda_account,
            minter: Some(minter),
            deploy_approval_pda: Some(deploy_approval_pda),
            deploy_approval_pda_account: Some(deploy_approval_pda_account),
            minter_roles: Some(minter_roles),
            minter_roles_account: Some(minter_roles_account),
        }
    }
}

pub fn deploy_remote_interchain_token_helper(
    salt: [u8; 32],
    destination_chain: String,
    gas_value: u64,
    ctx: DeployRemoteInterchainTokenContext,
) -> InstructionResult {
    let (gateway_root_pda, _) = Pubkey::find_program_address(
        &[axelar_solana_gateway_v2::seed_prefixes::GATEWAY_SEED],
        &axelar_solana_gateway_v2::ID,
    );

    let (gas_treasury, _) =
        Pubkey::find_program_address(&[Treasury::SEED_PREFIX], &axelar_solana_gas_service_v2::ID);

    let (call_contract_signing_pda, signing_pda_bump) =
        Pubkey::find_program_address(&[CALL_CONTRACT_SIGNING_SEED], &ctx.program_id);

    let (gateway_event_authority, _) =
        Pubkey::find_program_address(&[b"__event_authority"], &axelar_solana_gateway_v2::ID);

    let (gas_event_authority, _) =
        Pubkey::find_program_address(&[b"__event_authority"], &axelar_solana_gas_service_v2::ID);

    let (its_event_authority, its_event_authority_account, its_program_account) =
        get_event_authority_and_program_accounts(&ctx.program_id);

    let data = match ctx.minter {
        Some(minter) => axelar_solana_its_v2::instruction::DeployRemoteInterchainTokenWithMinter {
            salt,
            destination_chain: destination_chain.clone(),
            gas_value,
            signing_pda_bump,
            destination_minter: minter.to_bytes().into(),
        }
        .data(),
        None => axelar_solana_its_v2::instruction::DeployRemoteInterchainToken {
            salt,
            destination_chain: destination_chain.clone(),
            gas_value,
            signing_pda_bump,
        }
        .data(),
    };

    let ix = Instruction {
        program_id: ctx.program_id,
        accounts: axelar_solana_its_v2::accounts::DeployRemoteInterchainToken {
            payer: ctx.payer,
            deployer: ctx.deployer,
            token_mint: ctx.token_mint_pda,
            metadata_account: ctx.metadata_account,
            token_manager_pda: ctx.token_manager_pda,
            // optional minter accounts
            minter: ctx.minter,
            deploy_approval_pda: ctx.deploy_approval_pda,
            minter_roles: ctx.minter_roles,
            //
            gateway_root_pda,
            gateway_program: axelar_solana_gateway_v2::ID,
            gas_treasury,
            gas_service: axelar_solana_gas_service_v2::ID,
            system_program: system_program::ID,
            its_root_pda: ctx.its_root_pda,
            call_contract_signing_pda,
            its_program: ctx.program_id,
            gateway_event_authority,
            gas_event_authority,
            event_authority: its_event_authority,
            program: ctx.program_id,
        }
        .to_account_metas(None),
        data,
    };

    // Get the updated accounts from the first instruction result

    let updated_payer_account = ctx
        .result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == ctx.payer)
        .map(|(_, account)| account.clone())
        .unwrap_or_else(|| Account::new(9 * LAMPORTS_PER_SOL, 0, &system_program::ID));

    let updated_its_root_account = ctx.result.get_account(&ctx.its_root_pda).unwrap().clone();

    let updated_token_mint_account = ctx.result.get_account(&ctx.token_mint_pda).unwrap().clone();

    let updated_metadata_account = ctx
        .result
        .get_account(&ctx.metadata_account)
        .unwrap()
        .clone();

    let updated_token_manager_account = ctx
        .result
        .get_account(&ctx.token_manager_pda)
        .unwrap()
        .clone();

    // Accounts for the deploy remote instruction
    let accounts = vec![
        (ctx.payer, updated_payer_account),
        (
            ctx.deployer,
            Account::new(10 * LAMPORTS_PER_SOL, 0, &system_program::ID),
        ),
        (ctx.token_mint_pda, updated_token_mint_account),
        (ctx.metadata_account, updated_metadata_account),
        (ctx.token_manager_pda, updated_token_manager_account),
        // Optional minter accounts
        (
            ctx.minter.unwrap_or(ctx.program_id),
            Account::new(1_000_000_000, 0, &system_program::ID),
        ),
        (
            ctx.deploy_approval_pda.unwrap_or(ctx.program_id),
            ctx.deploy_approval_pda_account.unwrap_or(Account::new(
                1_000_000_000,
                0,
                &system_program::ID,
            )),
        ),
        (
            ctx.minter_roles.unwrap_or(ctx.program_id),
            ctx.minter_roles_account
                .unwrap_or(Account::new(1_000_000_000, 0, &system_program::ID)),
        ),
        //
        (gateway_root_pda, ctx.gateway_root_pda_account.clone()),
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
        (gas_treasury, ctx.treasury_pda.clone()),
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
        system_account_tuple(),
        (ctx.its_root_pda, updated_its_root_account),
        (
            call_contract_signing_pda,
            Account::new(0, 0, &ctx.program_id),
        ),
        (
            ctx.program_id,
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
        (ctx.program_id, its_program_account),
    ];

    ctx.mollusk.process_instruction(&ix, &accounts)
}

pub struct ApproveDeployRemoteInterchainTokenContext {
    mollusk: Mollusk,
    result: InstructionResult,
    minter: Pubkey,
    program_id: Pubkey,
    payer: Pubkey,
    token_manager_pda: Pubkey,
    minter_roles_pda: Pubkey,
    deploy_approval_pda: Pubkey,
}

impl ApproveDeployRemoteInterchainTokenContext {
    pub fn new(
        mollusk: Mollusk,
        result: InstructionResult,
        minter: Pubkey,
        program_id: Pubkey,
        payer: Pubkey,
        token_manager_pda: Pubkey,
        minter_roles_pda: Pubkey,
        deploy_approval_pda: Pubkey,
    ) -> Self {
        Self {
            mollusk,
            result,
            minter,
            program_id,
            payer,
            token_manager_pda,
            minter_roles_pda,
            deploy_approval_pda,
        }
    }
}

pub fn approve_deploy_remote_interchain_token_helper(
    deployer: Pubkey,
    salt: [u8; 32],
    destination_minter: Vec<u8>,
    destination_chain: String,
    ctx: ApproveDeployRemoteInterchainTokenContext,
) -> (InstructionResult, Mollusk) {
    let (event_authority, event_authority_account, program_account) =
        get_event_authority_and_program_accounts(&ctx.program_id);

    let approve_ix_data = axelar_solana_its_v2::instruction::ApproveDeployRemoteInterchainToken {
        deployer,
        salt,
        destination_chain: destination_chain.clone(),
        destination_minter: destination_minter.clone(),
    }
    .data();

    let approve_ix = Instruction {
        program_id: ctx.program_id,
        accounts: axelar_solana_its_v2::accounts::ApproveDeployRemoteInterchainToken {
            payer: ctx.payer,
            minter: ctx.minter,
            token_manager_pda: ctx.token_manager_pda,
            minter_roles: ctx.minter_roles_pda,
            deploy_approval_pda: ctx.deploy_approval_pda,
            system_program: system_program::ID,
            // for event CPI
            event_authority,
            program: ctx.program_id,
        }
        .to_account_metas(None),
        data: approve_ix_data,
    };

    let updated_payer_account = ctx
        .result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == ctx.payer)
        .map(|(_, account)| account.clone())
        .unwrap_or_else(|| Account::new(9 * LAMPORTS_PER_SOL, 0, &system_program::ID));

    let updated_token_manager_account = ctx
        .result
        .get_account(&ctx.token_manager_pda)
        .unwrap()
        .clone();

    let updated_minter_roles_account = ctx
        .result
        .get_account(&ctx.minter_roles_pda)
        .unwrap()
        .clone();

    let approve_accounts = vec![
        (ctx.payer, updated_payer_account),
        (ctx.minter, Account::new(0, 0, &system_program::ID)),
        (ctx.token_manager_pda, updated_token_manager_account),
        (ctx.minter_roles_pda, updated_minter_roles_account),
        (
            ctx.deploy_approval_pda,
            Account::new(0, 0, &system_program::ID),
        ),
        system_account_tuple(),
        // For event CPI
        (event_authority, event_authority_account),
        (ctx.program_id, program_account),
    ];

    (
        ctx.mollusk
            .process_instruction(&approve_ix, &approve_accounts),
        ctx.mollusk,
    )
}

pub fn register_canonical_interchain_token_helper(
    mollusk: &Mollusk,
    mint_data: Vec<u8>,
    mint_keypair: &Keypair,
    mint_authority: &Keypair,
    payer: Pubkey,
    payer_account: &Account,
    its_root_pda: Pubkey,
    its_root_account: &Account,
    program_id: Pubkey,
) -> InstructionResult {
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
            // for event cpi
            event_authority,
            program: program_id,
        }
        .to_account_metas(None),
        data: axelar_solana_its_v2::instruction::RegisterCanonicalInterchainToken {}.data(),
    };

    // Set up accounts
    let accounts = vec![
        (payer, payer_account.clone()),
        (metadata_account_pda, metadata_account),
        system_account_tuple(),
        (its_root_pda, its_root_account.clone()),
        (token_manager_pda, Account::new(0, 0, &system_program::ID)),
        (mint_pubkey, mint_account),
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
        // For event CPI
        (event_authority, event_authority_account),
        (program_id, program_account),
    ];

    mollusk.process_and_validate_instruction(&ix, &accounts, &[Check::success()])
}

pub fn initialize_mollusk() -> Mollusk {
    let program_id = axelar_solana_its_v2::id();
    let mut mollusk = setup_mollusk(&program_id, "axelar_solana_its_v2");

    mollusk.add_program(
        &mpl_token_metadata::programs::MPL_TOKEN_METADATA_ID,
        "../../programs/axelar-solana-its-v2/tests/mpl_token_metadata",
        &solana_sdk::bpf_loader_upgradeable::id(),
    );

    let spl_token_elf = mollusk_svm_programs_token::token::ELF;
    mollusk.add_program_with_elf_and_loader(
        &spl_token::ID,
        &spl_token_elf,
        &solana_sdk::bpf_loader_upgradeable::ID,
    );

    let token_2022_elf = mollusk_svm_programs_token::token2022::ELF;
    mollusk.add_program_with_elf_and_loader(
        &spl_token_2022::ID,
        &token_2022_elf,
        &solana_sdk::bpf_loader_upgradeable::ID,
    );

    let associated_token_elf = mollusk_svm_programs_token::associated_token::ELF;
    mollusk.add_program_with_elf_and_loader(
        &anchor_spl::associated_token::ID,
        &associated_token_elf,
        &solana_sdk::bpf_loader_upgradeable::ID,
    );

    mollusk.add_program(
        &axelar_solana_gas_service_v2::ID,
        "../../target/deploy/axelar_solana_gas_service_v2",
        &solana_sdk::bpf_loader_upgradeable::ID,
    );

    mollusk.add_program(
        &axelar_solana_gateway_v2::ID,
        "../../target/deploy/axelar_solana_gateway_v2",
        &solana_sdk::bpf_loader_upgradeable::ID,
    );

    mollusk
}

pub fn setup_operator(
    mollusk: &mut Mollusk,
    operator: Pubkey,
    operator_account: &Account,
) -> (Pubkey, Account) {
    let program_id = axelar_solana_operators::id();

    // Load the operators program into mollusk
    mollusk.add_program(
        &program_id,
        "axelar_solana_operators",
        &solana_sdk::bpf_loader_upgradeable::ID,
    );

    // Derive the registry PDA
    let (registry, _bump) = Pubkey::find_program_address(
        &[axelar_solana_operators::OperatorRegistry::SEED_PREFIX],
        &program_id,
    );
    // Derive the operator PDA
    let (operator_pda, _bump) = Pubkey::find_program_address(
        &[
            axelar_solana_operators::OperatorAccount::SEED_PREFIX,
            operator.as_ref(),
        ],
        &program_id,
    );

    // Initialize the registry instruction
    let ix1 = Instruction {
        program_id,
        accounts: axelar_solana_operators::accounts::Initialize {
            payer: operator,
            owner: operator,
            registry,
            system_program: solana_sdk::system_program::ID,
        }
        .to_account_metas(None),
        data: axelar_solana_operators::instruction::Initialize {}.data(),
    };

    let checks1 = vec![
        Check::success(),
        Check::account(&registry)
            .space(OperatorRegistry::DISCRIMINATOR.len() + OperatorRegistry::INIT_SPACE)
            .build(),
        Check::all_rent_exempt(),
    ];

    // Add operator instruction
    let ix2 = Instruction {
        program_id,
        accounts: axelar_solana_operators::accounts::AddOperator {
            owner: operator,
            operator_to_add: operator,
            registry,
            operator_account: operator_pda,
            system_program: solana_sdk::system_program::ID,
        }
        .to_account_metas(None),
        data: axelar_solana_operators::instruction::AddOperator {}.data(),
    };

    let checks2 = vec![
        Check::success(),
        Check::account(&operator_pda)
            .space(OperatorAccount::DISCRIMINATOR.len() + OperatorAccount::INIT_SPACE)
            .build(),
        Check::all_rent_exempt(),
    ];

    // List accounts
    let accounts = vec![
        (operator, operator_account.clone()),
        (
            registry,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
        (
            operator_pda,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
        keyed_account_for_system_program(),
    ];

    let result = mollusk.process_and_validate_instruction_chain(
        &[
            // Initialize the registry
            (&ix1, &checks1),
            // Add the operator
            (&ix2, &checks2),
        ],
        &accounts,
    );

    let operator_pda_account = result
        .get_account(&operator_pda)
        .expect("Operator PDA should exist");

    (operator_pda, operator_pda_account.clone())
}

pub fn init_gas_service(
    mollusk: &Mollusk,
    operator: Pubkey,
    operator_account: &Account,
    operator_pda: Pubkey,
    operator_pda_account: &Account,
) -> (Pubkey, Account) {
    let program_id = axelar_solana_gas_service_v2::id();

    // Derive the treasury PDA
    let (treasury, _bump) = Pubkey::find_program_address(&[Treasury::SEED_PREFIX], &program_id);

    let ix = Instruction {
        program_id,
        accounts: axelar_solana_gas_service_v2::accounts::Initialize {
            payer: operator,
            operator,
            operator_pda,
            treasury,
            system_program: solana_sdk::system_program::ID,
        }
        .to_account_metas(None),
        data: axelar_solana_gas_service_v2::instruction::Initialize {}.data(),
    };

    let accounts = vec![
        (operator, operator_account.clone()),
        (operator_pda, operator_pda_account.clone()),
        (
            treasury,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
        keyed_account_for_system_program(),
    ];

    let checks = vec![
        Check::success(),
        Check::account(&treasury)
            .space(Treasury::DISCRIMINATOR.len() + Treasury::INIT_SPACE)
            .build(),
        Check::all_rent_exempt(),
    ];

    let result = mollusk.process_and_validate_instruction(&ix, &accounts, &checks);

    let treasury_pda = result
        .get_account(&treasury)
        .expect("Treasury PDA should exist");

    (treasury, treasury_pda.clone())
}

pub fn init_its_service(
    mollusk: &Mollusk,
    payer: Pubkey,
    payer_account: &Account,
    upgrade_authority: Pubkey,
    operator: Pubkey,
    operator_account: &Account,
    chain_name: String,
    its_hub_address: String,
) -> (Pubkey, Account, Pubkey, Account, Pubkey, Account) {
    let program_id = axelar_solana_its_v2::id();

    // Derive the program data PDA for the upgradeable program
    let (program_data, _bump) = Pubkey::find_program_address(
        &[program_id.as_ref()],
        &solana_sdk::bpf_loader_upgradeable::ID,
    );
    let its_elf = mollusk_svm::file::load_program_elf("axelar_solana_its_v2");
    let program_data_account = create_program_data_account(&its_elf, upgrade_authority);

    if payer != upgrade_authority {
        println!("[WARNING] Initialize will fail since payer is not the upgrade authority");
    }

    // Derive the ITS root PDA
    let (its_root_pda, _bump) =
        Pubkey::find_program_address(&[InterchainTokenService::SEED_PREFIX], &program_id);

    // Derive the user roles PDA
    let (user_roles_pda, _bump) = Pubkey::find_program_address(
        &UserRoles::pda_seeds(&its_root_pda, &operator)[..],
        &program_id,
    );

    let ix = Instruction {
        program_id,
        accounts: axelar_solana_its_v2::accounts::Initialize {
            payer,
            program_data,
            its_root_pda,
            system_program: solana_sdk::system_program::ID,
            operator,
            user_roles_account: user_roles_pda,
        }
        .to_account_metas(None),
        data: axelar_solana_its_v2::instruction::Initialize {
            chain_name,
            its_hub_address,
        }
        .data(),
    };

    let accounts = vec![
        (payer, payer_account.clone()),
        (program_data, program_data_account.clone()),
        (
            its_root_pda,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
        keyed_account_for_system_program(),
        (operator, operator_account.clone()),
        (
            user_roles_pda,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
    ];

    let checks = vec![
        Check::success(),
        Check::account(&its_root_pda)
            .space(InterchainTokenService::DISCRIMINATOR.len() + InterchainTokenService::INIT_SPACE)
            .build(),
        Check::account(&user_roles_pda)
            .space(UserRoles::DISCRIMINATOR.len() + UserRoles::INIT_SPACE)
            .build(),
        Check::all_rent_exempt(),
    ];

    let result = mollusk.process_and_validate_instruction(&ix, &accounts, &checks);

    let its_root_account = result
        .get_account(&its_root_pda)
        .expect("ITS root PDA should exist");

    let user_roles_account = result
        .get_account(&user_roles_pda)
        .expect("User roles PDA should exist");

    let user_roles_data = UserRoles::try_deserialize(&mut user_roles_account.data.as_slice())
        .expect("Failed to deserialize roles data");
    assert_eq!(user_roles_data.roles, Roles::OPERATOR);

    (
        its_root_pda,
        its_root_account.clone(),
        user_roles_pda,
        user_roles_account.clone(),
        program_data,
        program_data_account,
    )
}

pub fn init_its_service_with_ethereum_trusted(
    mollusk: &Mollusk,
    payer: Pubkey,
    payer_account: &Account,
    upgrade_authority: Pubkey,
    operator: Pubkey,
    operator_account: &Account,
    chain_name: String,
    its_hub_address: String,
) -> (Pubkey, Account) {
    let program_id = axelar_solana_its_v2::id();

    // First initialize the ITS service
    let (
        its_root_pda,
        its_root_account,
        _user_roles_pda,
        _user_roles_account,
        program_data,
        program_data_account,
    ) = init_its_service(
        mollusk,
        payer,
        payer_account,
        upgrade_authority,
        operator,
        operator_account,
        chain_name,
        its_hub_address,
    );

    let (event_authority, event_authority_account, program_account) =
        get_event_authority_and_program_accounts(&program_id);

    // Add ethereum as a trusted chain
    let trusted_chain_name = "ethereum".to_string();

    let ix = Instruction {
        program_id,
        accounts: axelar_solana_its_v2::accounts::SetTrustedChain {
            payer,
            user_roles: None,
            program_data: Some(program_data),
            its_root_pda,
            system_program: solana_sdk::system_program::ID,
            event_authority: event_authority,
            program: program_id,
        }
        .to_account_metas(None),
        data: axelar_solana_its_v2::instruction::SetTrustedChain {
            chain_name: trusted_chain_name.clone(),
        }
        .data(),
    };

    let accounts = vec![
        (payer, payer_account.clone()),
        (program_data, program_data_account.clone()),
        (its_root_pda, its_root_account.clone()),
        keyed_account_for_system_program(),
        (event_authority, event_authority_account),
        (program_id, program_account),
    ];

    let checks = vec![Check::success()];

    let result = mollusk.process_and_validate_instruction(&ix, &accounts, &checks);

    let updated_its_account = result
        .get_account(&its_root_pda)
        .expect("ITS root PDA should exist");

    // Verify ethereum was added as trusted chain
    let updated_its_data =
        InterchainTokenService::try_deserialize(&mut updated_its_account.data.as_slice())
            .expect("Failed to deserialize updated ITS data");

    assert!(updated_its_data.is_trusted_chain("ethereum"));

    (its_root_pda, updated_its_account.clone())
}
