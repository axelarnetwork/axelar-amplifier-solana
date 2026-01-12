use anchor_lang::solana_program;
use anchor_lang::InstructionData;
use anchor_lang::ToAccountMetas;
use mollusk_svm::program::keyed_account_for_system_program;
use mollusk_svm::result::Check;
use mollusk_svm::result::InstructionResult;
use mollusk_svm::Mollusk;
use mollusk_test_utils::get_event_authority_and_program_accounts;
use solana_sdk::{account::Account, pubkey::Pubkey};

pub struct DeployRemoteCanonicalTokenContext {
    pub mollusk: Mollusk,
    pub deployer: (Pubkey, Account),
    pub mint: (Pubkey, Account),
    pub metadata: (Pubkey, Account),
    pub token_manager: (Pubkey, Account),
    pub gateway_root: (Pubkey, Account),
    pub gas_treasury: (Pubkey, Account),
    pub its_root: (Pubkey, Account),
}

impl DeployRemoteCanonicalTokenContext {
    pub fn new(
        mollusk: Mollusk,
        deployer: (Pubkey, Account),
        mint: (Pubkey, Account),
        metadata: (Pubkey, Account),
        token_manager: (Pubkey, Account),
        gateway_root: (Pubkey, Account),
        gas_treasury: (Pubkey, Account),
        its_root: (Pubkey, Account),
    ) -> Self {
        Self {
            mollusk,
            deployer,
            mint,
            metadata,
            token_manager,
            gateway_root,
            gas_treasury,
            its_root,
        }
    }
}

pub fn deploy_remote_canonical_token_helper(
    ctx: DeployRemoteCanonicalTokenContext,
    destination_chain: String,
    gas_value: u64,
    checks: Vec<Check>,
) -> InstructionResult {
    let program_id = solana_axelar_its::id();

    // Get call contract signing PDA
    let (call_contract_signing_pda, _) =
        solana_axelar_gateway::CallContractSigner::find_pda(&program_id);

    // Get event authorities
    let (gateway_event_authority, _, _) =
        get_event_authority_and_program_accounts(&solana_axelar_gateway::ID);

    let (gas_event_authority, _, _) =
        get_event_authority_and_program_accounts(&solana_axelar_gas_service::ID);

    let (event_authority, event_authority_account, program_account) =
        get_event_authority_and_program_accounts(&program_id);

    // Create the deploy remote canonical instruction
    let deploy_remote_ix = solana_program::instruction::Instruction {
        program_id,
        accounts: solana_axelar_its::accounts::DeployRemoteCanonicalInterchainToken {
            payer: ctx.deployer.0,
            token_mint: ctx.mint.0,
            metadata_account: ctx.metadata.0,
            token_manager_pda: ctx.token_manager.0,
            gateway_root_pda: ctx.gateway_root.0,
            gateway_program: solana_axelar_gateway::ID,
            system_program: solana_sdk_ids::system_program::ID,
            its_root_pda: ctx.its_root.0,
            call_contract_signing_pda,
            gateway_event_authority,
            gas_treasury: ctx.gas_treasury.0,
            gas_service: solana_axelar_gas_service::id(),
            gas_event_authority,
            event_authority,
            program: program_id,
        }
        .to_account_metas(None),
        data: solana_axelar_its::instruction::DeployRemoteCanonicalInterchainToken {
            destination_chain,
            gas_value,
        }
        .data(),
    };

    // Set up accounts for deploy remote canonical instruction
    let deploy_accounts = vec![
        (ctx.deployer.0, ctx.deployer.1),
        (ctx.mint.0, ctx.mint.1),
        (ctx.metadata.0, ctx.metadata.1),
        (ctx.token_manager.0, ctx.token_manager.1),
        (ctx.gateway_root.0, ctx.gateway_root.1),
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
        (ctx.gas_treasury.0, ctx.gas_treasury.1),
        (
            solana_axelar_gas_service::id(),
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
            Account::new(0, 0, &solana_sdk_ids::system_program::ID),
        ),
        (
            program_id,
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
        // For event CPI
        (event_authority, event_authority_account),
        (program_id, program_account),
    ];

    ctx.mollusk
        .process_and_validate_instruction(&deploy_remote_ix, &deploy_accounts, &checks)
}
