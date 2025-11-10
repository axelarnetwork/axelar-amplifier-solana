use anchor_lang::{InstructionData, ToAccountMetas};
use mollusk_svm::program::keyed_account_for_system_program;
use mollusk_svm::result::Check;
use mollusk_svm::Mollusk;
use mollusk_test_utils::get_event_authority_and_program_accounts;
use solana_axelar_its::state::token_manager::Type;
use solana_sdk::{account::Account, instruction::Instruction, pubkey::Pubkey};

pub struct LinkTokenContext {
    pub mollusk: Mollusk,
    pub payer: Pubkey,
    pub payer_account: Account,
    pub deployer: Pubkey,
    pub deployer_account: Account,
    pub its_root_pda: Pubkey,
    pub its_root_account: Account,
    pub token_manager_pda: Pubkey,
    pub token_manager_account: Account,
    pub gateway_root_pda: Pubkey,
    pub gateway_root_pda_account: Account,
    pub gas_treasury: Pubkey,
    pub treasury_account: Account,
    pub program_id: Pubkey,
}

pub struct LinkTokenParams {
    pub salt: [u8; 32],
    pub destination_chain: String,
    pub destination_token_address: Vec<u8>,
    pub token_manager_type: Type,
    pub link_params: Vec<u8>,
    pub gas_value: u64,
}

pub struct LinkTokenResult {
    pub instruction: Instruction,
    pub accounts: Vec<(Pubkey, Account)>,
    pub result: mollusk_svm::result::InstructionResult,
}

pub fn execute_link_token_helper(
    ctx: LinkTokenContext,
    params: LinkTokenParams,
    checks: Vec<Check>,
) -> LinkTokenResult {
    let program_id = ctx.program_id;

    // Derive required PDAs
    let (call_contract_signing_pda, _) = Pubkey::find_program_address(
        &[solana_axelar_gateway::seed_prefixes::CALL_CONTRACT_SIGNING_SEED],
        &program_id,
    );

    let (gateway_event_authority, _, _) =
        get_event_authority_and_program_accounts(&solana_axelar_gateway::ID);

    let (gas_event_authority, _, _) =
        get_event_authority_and_program_accounts(&solana_axelar_gas_service::ID);

    let (event_authority, _, _) = get_event_authority_and_program_accounts(&program_id);

    // Create link token instruction
    let link_instruction_data = solana_axelar_its::instruction::LinkToken {
        salt: params.salt,
        destination_chain: params.destination_chain.clone(),
        destination_token_address: params.destination_token_address.clone(),
        token_manager_type: params.token_manager_type,
        link_params: params.link_params.clone(),
        gas_value: params.gas_value,
    };

    // Build accounts
    let link_accounts = solana_axelar_its::accounts::LinkToken {
        payer: ctx.payer,
        deployer: ctx.deployer,
        its_root_pda: ctx.its_root_pda,
        token_manager_pda: ctx.token_manager_pda,
        gateway_root_pda: ctx.gateway_root_pda,
        gateway_program: solana_axelar_gateway::ID,
        system_program: solana_sdk::system_program::ID,
        call_contract_signing_pda,
        gateway_event_authority,
        gas_service_accounts: solana_axelar_its::accounts::GasServiceAccounts {
            gas_treasury: ctx.gas_treasury,
            gas_service: solana_axelar_gas_service::ID,
            gas_event_authority,
        },
        // for event cpi
        event_authority,
        program: program_id,
    };

    let link_ix = Instruction {
        program_id,
        accounts: link_accounts.to_account_metas(None),
        data: link_instruction_data.data(),
    };

    // Setup accounts for mollusk
    let link_mollusk_accounts = vec![
        (ctx.payer, ctx.payer_account),
        (ctx.deployer, ctx.deployer_account),
        (ctx.token_manager_pda, ctx.token_manager_account.clone()),
        (ctx.gateway_root_pda, ctx.gateway_root_pda_account.clone()),
        (
            solana_axelar_gateway::ID,
            Account {
                lamports: 1,
                data: vec![],
                owner: solana_sdk::bpf_loader_upgradeable::ID,
                executable: true,
                rent_epoch: 0,
            },
        ),
        (ctx.gas_treasury, ctx.treasury_account),
        (
            solana_axelar_gas_service::ID,
            Account {
                lamports: 1,
                data: vec![],
                owner: solana_sdk::bpf_loader_upgradeable::ID,
                executable: true,
                rent_epoch: 0,
            },
        ),
        keyed_account_for_system_program(),
        (ctx.its_root_pda, ctx.its_root_account),
        (
            call_contract_signing_pda,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
        (
            gateway_event_authority,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
        (
            gas_event_authority,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
        // For event CPI
        (
            event_authority,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
        (
            program_id,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
    ];

    // Execute instruction
    let result =
        ctx.mollusk
            .process_and_validate_instruction(&link_ix, &link_mollusk_accounts, &checks);

    LinkTokenResult {
        instruction: link_ix,
        accounts: link_mollusk_accounts,
        result,
    }
}
