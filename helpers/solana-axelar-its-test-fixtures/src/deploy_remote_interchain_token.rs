use anchor_lang::{InstructionData, ToAccountMetas};
use mollusk_svm::program::keyed_account_for_system_program;
use mollusk_svm::result::Check;
use mollusk_svm::{result::InstructionResult, Mollusk};
use mollusk_test_utils::get_event_authority_and_program_accounts;
use solana_axelar_gas_service::state::Treasury;
use solana_axelar_gateway::CallContractSigner;
use solana_sdk::{
    account::Account, instruction::Instruction, native_token::LAMPORTS_PER_SOL, pubkey::Pubkey,
};

pub struct DeployRemoteInterchainTokenContext {
    mollusk: Mollusk,
    program_id: Pubkey,
    payer: (Pubkey, Account),
    deployer: Pubkey,
    token_mint: (Pubkey, Account),
    metadata: (Pubkey, Account),
    token_manager: (Pubkey, Account),
    its_root: (Pubkey, Account),
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
        mollusk: Mollusk,
        program_id: Pubkey,
        payer: (Pubkey, Account),
        deployer: Pubkey,
        token_mint: (Pubkey, Account),
        metadata: (Pubkey, Account),
        token_manager: (Pubkey, Account),
        its_root: (Pubkey, Account),
        treasury_pda: Account,
        gateway_root_pda_account: Account,
    ) -> Self {
        Self {
            mollusk,
            program_id,
            payer,
            deployer,
            token_mint,
            metadata,
            token_manager,
            its_root,
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
        mollusk: Mollusk,
        program_id: Pubkey,
        payer: (Pubkey, Account),
        deployer: Pubkey,
        token_mint: (Pubkey, Account),
        metadata: (Pubkey, Account),
        token_manager: (Pubkey, Account),
        its_root: (Pubkey, Account),
        treasury_pda: Account,
        gateway_root_pda_account: Account,
        minter: Pubkey,
        deploy_approval_pda: Pubkey,
        deploy_approval_pda_account: Account,
        minter_roles: Pubkey,
        minter_roles_account: Account,
    ) -> Self {
        Self {
            mollusk,
            program_id,
            payer,
            deployer,
            token_mint,
            metadata,
            token_manager,
            its_root,
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
    checks: Vec<Check>,
) -> InstructionResult {
    let (gateway_root_pda, _) = Pubkey::find_program_address(
        &[solana_axelar_gateway::seed_prefixes::GATEWAY_SEED],
        &solana_axelar_gateway::ID,
    );

    let (gas_treasury, _) =
        Pubkey::find_program_address(&[Treasury::SEED_PREFIX], &solana_axelar_gas_service::ID);

    let (call_contract_signing_pda, _signing_pda_bump) =
        CallContractSigner::find_pda(&ctx.program_id);

    let (gateway_event_authority, _) =
        Pubkey::find_program_address(&[b"__event_authority"], &solana_axelar_gateway::ID);

    let (gas_event_authority, _) =
        Pubkey::find_program_address(&[b"__event_authority"], &solana_axelar_gas_service::ID);

    let (its_event_authority, its_event_authority_account, its_program_account) =
        get_event_authority_and_program_accounts(&ctx.program_id);

    let data = solana_axelar_its::instruction::DeployRemoteInterchainToken {
        salt,
        destination_chain: destination_chain.clone(),
        gas_value,
    }
    .data();

    let ix = Instruction {
        program_id: ctx.program_id,
        accounts: solana_axelar_its::accounts::DeployRemoteInterchainToken {
            payer: ctx.payer.0,
            deployer: ctx.deployer,
            token_mint: ctx.token_mint.0,
            metadata_account: ctx.metadata.0,
            token_manager_pda: ctx.token_manager.0,
            gateway_root_pda,
            gateway_program: solana_axelar_gateway::ID,
            gas_service: solana_axelar_gas_service::ID,
            gas_treasury,
            gas_event_authority,
            system_program: solana_sdk_ids::system_program::ID,
            its_root_pda: ctx.its_root.0,
            call_contract_signing_pda,
            gateway_event_authority,
            event_authority: its_event_authority,
            program: ctx.program_id,
        }
        .to_account_metas(None),
        data,
    };

    // Accounts for the deploy remote instruction
    let accounts = vec![
        (ctx.payer.0, ctx.payer.1),
        (
            ctx.deployer,
            Account::new(
                10 * LAMPORTS_PER_SOL,
                0,
                &solana_sdk_ids::system_program::ID,
            ),
        ),
        (ctx.token_mint.0, ctx.token_mint.1),
        (ctx.metadata.0, ctx.metadata.1),
        (ctx.token_manager.0, ctx.token_manager.1),
        // Optional minter accounts
        (
            ctx.minter.unwrap_or(ctx.program_id),
            Account::new(1_000_000_000, 0, &solana_sdk_ids::system_program::ID),
        ),
        (
            ctx.deploy_approval_pda.unwrap_or(ctx.program_id),
            ctx.deploy_approval_pda_account.unwrap_or(Account::new(
                1_000_000_000,
                0,
                &solana_sdk_ids::system_program::ID,
            )),
        ),
        (
            ctx.minter_roles.unwrap_or(ctx.program_id),
            ctx.minter_roles_account.unwrap_or(Account::new(
                1_000_000_000,
                0,
                &solana_sdk_ids::system_program::ID,
            )),
        ),
        //
        (gateway_root_pda, ctx.gateway_root_pda_account.clone()),
        (
            solana_axelar_gateway::ID,
            Account {
                lamports: 1,
                data: vec![],
                owner: solana_sdk_ids::bpf_loader_upgradeable::ID,
                executable: true,
                rent_epoch: 0,
            },
        ),
        (gas_treasury, ctx.treasury_pda.clone()),
        (
            solana_axelar_gas_service::ID,
            Account {
                lamports: 1,
                data: vec![],
                owner: solana_sdk_ids::bpf_loader_upgradeable::ID,
                executable: true,
                rent_epoch: 0,
            },
        ),
        keyed_account_for_system_program(),
        (ctx.its_root.0, ctx.its_root.1),
        (
            call_contract_signing_pda,
            Account::new(0, 0, &ctx.program_id),
        ),
        (
            ctx.program_id,
            Account {
                lamports: 1,
                data: vec![],
                owner: solana_sdk_ids::bpf_loader_upgradeable::ID,
                executable: true,
                rent_epoch: 0,
            },
        ),
        (
            gateway_event_authority,
            Account::new(0, 0, &solana_sdk_ids::system_program::ID),
        ),
        (
            gas_event_authority,
            Account::new(0, 0, &solana_sdk_ids::system_program::ID),
        ),
        // For event cpi
        (its_event_authority, its_event_authority_account),
        (ctx.program_id, its_program_account),
    ];

    ctx.mollusk
        .process_and_validate_instruction(&ix, &accounts, &checks)
}
