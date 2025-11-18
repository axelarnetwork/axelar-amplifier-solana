use anchor_lang::{InstructionData, ToAccountMetas};
use anchor_spl::token_2022::spl_token_2022;
use mollusk_svm::program::keyed_account_for_system_program;
use mollusk_svm::result::Check;
use mollusk_svm::{result::InstructionResult, Mollusk};
use mollusk_test_utils::get_event_authority_and_program_accounts;
use solana_axelar_gateway::seed_prefixes::CALL_CONTRACT_SIGNING_SEED;
use solana_sdk::{
    account::Account, instruction::Instruction, native_token::LAMPORTS_PER_SOL, pubkey::Pubkey,
};

pub struct InterchainTransferContext {
    payer: (Pubkey, Account),
    authority: (Pubkey, Account),
    its_root_pda: (Pubkey, Account),
    deployer_ata: (Pubkey, Account),
    token_mint_pda: (Pubkey, Account),
    token_manager_pda: (Pubkey, Account),
    token_manager_ata: (Pubkey, Account),
    gateway_root_pda: (Pubkey, Account),
    treasury_pda: (Pubkey, Account),
    mollusk: Mollusk,
}

impl InterchainTransferContext {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        payer: (Pubkey, Account),
        authority: (Pubkey, Account),
        its_root_pda: (Pubkey, Account),
        deployer_ata: (Pubkey, Account),
        token_mint_pda: (Pubkey, Account),
        token_manager_pda: (Pubkey, Account),
        token_manager_ata: (Pubkey, Account),
        gateway_root_pda: (Pubkey, Account),
        treasury_pda: (Pubkey, Account),
        mollusk: Mollusk,
    ) -> Self {
        Self {
            payer,
            authority,
            its_root_pda,
            deployer_ata,
            token_mint_pda,
            token_manager_pda,
            token_manager_ata,
            gateway_root_pda,
            treasury_pda,
            mollusk,
        }
    }
}

pub fn perform_interchain_transfer(
    ctx: InterchainTransferContext,
    token_id: [u8; 32],
    destination_chain: String,
    destination_address: Vec<u8>,
    transfer_amount: u64,
    gas_value: u64,
    checks: Vec<Check>,
) -> (InstructionResult, Mollusk) {
    let program_id = solana_axelar_its::ID;
    let (signing_pda, _) =
        Pubkey::find_program_address(&[CALL_CONTRACT_SIGNING_SEED], &solana_axelar_its::ID);

    let (gas_event_authority, _, _) =
        get_event_authority_and_program_accounts(&solana_axelar_gas_service::ID);

    let (gateway_event_authority, _, _) =
        get_event_authority_and_program_accounts(&solana_axelar_gateway::ID);

    let (its_event_authority, event_authority_account, its_program_account) =
        get_event_authority_and_program_accounts(&program_id);

    let accounts = solana_axelar_its::accounts::InterchainTransfer {
        payer: ctx.payer.0,
        authority: ctx.authority.0,
        its_root_pda: ctx.its_root_pda.0,
        authority_token_account: ctx.deployer_ata.0,
        token_mint: ctx.token_mint_pda.0,
        token_manager_pda: ctx.token_manager_pda.0,
        token_manager_ata: ctx.token_manager_ata.0,
        token_program: spl_token_2022::ID,
        gateway_root_pda: ctx.gateway_root_pda.0,
        gateway_event_authority,
        gateway_program: solana_axelar_gateway::ID,
        gas_treasury: ctx.treasury_pda.0,
        gas_service: solana_axelar_gas_service::ID,
        gas_event_authority,
        system_program: solana_sdk::system_program::ID,
        signing_pda,
        event_authority: its_event_authority,
        program: program_id,
    };

    let instruction_data = solana_axelar_its::instruction::InterchainTransfer {
        token_id,
        destination_chain: destination_chain.clone(),
        destination_address: destination_address.clone(),
        amount: transfer_amount,
        gas_value,
        source_id: None,
        pda_seeds: None,
        data: None,
    };

    let instruction = Instruction {
        program_id,
        accounts: accounts.to_account_metas(None),
        data: instruction_data.data(),
    };

    let transfer_accounts = vec![
        ctx.payer,
        ctx.authority,
        ctx.its_root_pda,
        ctx.deployer_ata,
        ctx.token_mint_pda,
        ctx.token_manager_pda,
        ctx.token_manager_ata,
        mollusk_svm_programs_token::token2022::keyed_account(),
        ctx.gateway_root_pda,
        (
            gateway_event_authority,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
        (
            solana_axelar_gateway::ID,
            Account {
                lamports: LAMPORTS_PER_SOL,
                data: vec![],
                owner: solana_sdk::bpf_loader_upgradeable::id(),
                executable: true,
                rent_epoch: 0,
            },
        ),
        ctx.treasury_pda,
        (
            solana_axelar_gas_service::ID,
            Account {
                lamports: LAMPORTS_PER_SOL,
                data: vec![],
                owner: solana_sdk::bpf_loader_upgradeable::id(),
                executable: true,
                rent_epoch: 0,
            },
        ),
        (
            gas_event_authority,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
        keyed_account_for_system_program(),
        (
            signing_pda,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
        (program_id, its_program_account.clone()),
        (its_event_authority, event_authority_account),
        (program_id, its_program_account),
    ];

    (
        ctx.mollusk
            .process_and_validate_instruction(&instruction, &transfer_accounts, &checks),
        ctx.mollusk,
    )
}
