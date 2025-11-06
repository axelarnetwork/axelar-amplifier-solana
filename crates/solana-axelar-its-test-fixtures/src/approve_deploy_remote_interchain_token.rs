use anchor_lang::{InstructionData, ToAccountMetas};
use mollusk_svm::program::keyed_account_for_system_program;
use mollusk_svm::{result::InstructionResult, Mollusk};
use mollusk_test_utils::get_event_authority_and_program_accounts;
use solana_sdk::{
    account::Account, instruction::Instruction, native_token::LAMPORTS_PER_SOL, pubkey::Pubkey,
};

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

    let approve_ix_data = solana_axelar_its::instruction::ApproveDeployRemoteInterchainToken {
        deployer,
        salt,
        destination_chain: destination_chain.clone(),
        destination_minter: destination_minter.clone(),
    }
    .data();

    let approve_ix = Instruction {
        program_id: ctx.program_id,
        accounts: solana_axelar_its::accounts::ApproveDeployRemoteInterchainToken {
            payer: ctx.payer,
            minter: ctx.minter,
            token_manager_pda: ctx.token_manager_pda,
            minter_roles: ctx.minter_roles_pda,
            deploy_approval_pda: ctx.deploy_approval_pda,
            system_program: solana_sdk::system_program::ID,
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
        .unwrap_or_else(|| Account::new(9 * LAMPORTS_PER_SOL, 0, &solana_sdk::system_program::ID));

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
        (
            ctx.minter,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
        (ctx.token_manager_pda, updated_token_manager_account),
        (ctx.minter_roles_pda, updated_minter_roles_account),
        (
            ctx.deploy_approval_pda,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
        keyed_account_for_system_program(),
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
