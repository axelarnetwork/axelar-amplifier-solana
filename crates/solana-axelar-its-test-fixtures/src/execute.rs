use anchor_lang::{InstructionData, ToAccountMetas};
use mollusk_svm::result::Check;
use mollusk_svm::Mollusk;
use mollusk_test_utils::get_event_authority_and_program_accounts;
use solana_axelar_gateway::{Message, ID as GATEWAY_PROGRAM_ID};
use solana_axelar_its::state::TokenManager;
use solana_sdk::{account::Account, instruction::Instruction, pubkey::Pubkey};

use crate::{create_rent_sysvar_data, get_message_signing_pda};

/// Context for executing ITS instructions
pub struct ExecuteTestContext {
    pub mollusk: Mollusk,
    pub gateway_root_pda: Pubkey,
    pub gateway_root_pda_account: Account,
    pub its_root_pda: Pubkey,
    pub its_root_account: Account,
    pub payer: Pubkey,
    pub payer_account: Account,
    pub program_id: Pubkey,
}

/// Parameters for the execute instruction
pub struct ExecuteTestParams {
    pub message: Message,
    pub payload: Vec<u8>,
    pub token_id: [u8; 32],
    pub incoming_message_pda: Pubkey,
    pub incoming_message_account_data: Vec<u8>,
}

/// Account configuration for execute instruction
pub struct ExecuteTestAccounts {
    /// Core ITS accounts that are always needed
    pub core_accounts: Vec<(Pubkey, Account)>,
    /// Extra accounts specific to the instruction type (e.g., for deploy, link, transfer)
    pub extra_accounts: Vec<(Pubkey, Account)>,
    /// Extra account metas for remaining accounts
    pub extra_account_metas: Vec<anchor_lang::solana_program::instruction::AccountMeta>,
}

/// Result of the execute test
pub struct ExecuteTestResult {
    pub result: mollusk_svm::result::InstructionResult,
    pub instruction: Instruction,
    pub all_accounts: Vec<(Pubkey, Account)>,
}

/// Helper function to execute ITS instructions with common setup
pub fn execute_its_instruction(
    context: ExecuteTestContext,
    params: ExecuteTestParams,
    accounts_config: ExecuteTestAccounts,
    checks: Vec<Check>,
) -> ExecuteTestResult {
    let ExecuteTestParams {
        message,
        payload,
        token_id,
        incoming_message_pda,
        incoming_message_account_data,
    } = params;

    let ExecuteTestAccounts {
        core_accounts,
        extra_accounts,
        extra_account_metas,
    } = accounts_config;

    // Derive required PDAs
    let (token_manager_pda, _) = TokenManager::find_pda(token_id, context.its_root_pda);

    let (signing_pda, _) = get_message_signing_pda(&message);

    let (gateway_event_authority, _, _) =
        get_event_authority_and_program_accounts(&GATEWAY_PROGRAM_ID);

    let (its_event_authority, event_authority_account, its_program_account) =
        get_event_authority_and_program_accounts(&context.program_id);

    // Create instruction data
    let instruction_data = solana_axelar_its::instruction::Execute {
        message: message.clone(),
        payload,
    };

    // Create base accounts
    let executable_accounts = solana_axelar_its::accounts::AxelarExecuteAccounts {
        incoming_message_pda,
        signing_pda,
        gateway_root_pda: context.gateway_root_pda,
        event_authority: gateway_event_authority,
        axelar_gateway_program: GATEWAY_PROGRAM_ID,
    };

    let base_accounts = solana_axelar_its::accounts::Execute {
        executable: executable_accounts,
        payer: context.payer,
        its_root_pda: context.its_root_pda,
        token_manager_pda,
        token_mint: core_accounts[0].0, // First core account should be token_mint
        token_manager_ata: core_accounts[1].0, // Second should be token_manager_ata
        token_program: anchor_spl::token_2022::spl_token_2022::id(),
        associated_token_program: anchor_spl::associated_token::spl_associated_token_account::id(),
        system_program: solana_sdk::system_program::ID,
        event_authority: its_event_authority,
        program: context.program_id,
    };

    let mut account_metas = base_accounts.to_account_metas(None);
    account_metas.extend(extra_account_metas);

    let execute_instruction = Instruction {
        program_id: context.program_id,
        accounts: account_metas,
        data: instruction_data.data(),
    };

    // Prepare incoming message account
    let incoming_message_account = Account {
        lamports: context
            .mollusk
            .sysvars
            .rent
            .minimum_balance(incoming_message_account_data.len()),
        data: incoming_message_account_data,
        owner: GATEWAY_PROGRAM_ID,
        executable: false,
        rent_epoch: 0,
    };

    // Build complete accounts list
    let mut execute_accounts = vec![
        // AxelarExecuteAccounts
        (incoming_message_pda, incoming_message_account),
        (
            signing_pda,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
        (context.gateway_root_pda, context.gateway_root_pda_account),
        (
            gateway_event_authority,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
        (
            GATEWAY_PROGRAM_ID,
            Account {
                lamports: solana_sdk::native_token::LAMPORTS_PER_SOL,
                data: vec![],
                owner: solana_sdk::bpf_loader_upgradeable::id(),
                executable: true,
                rent_epoch: 0,
            },
        ),
        // Base ITS accounts
        (context.payer, context.payer_account.clone()),
        (context.its_root_pda, context.its_root_account),
        (
            token_manager_pda,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
    ];

    // Add core accounts (token_mint, token_manager_ata, etc.)
    execute_accounts.extend(core_accounts);

    // Add system accounts
    execute_accounts.extend(vec![
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
        mollusk_svm::program::keyed_account_for_system_program(),
    ]);

    // Add extra accounts
    execute_accounts.extend(extra_accounts);

    // Add event CPI accounts
    execute_accounts.extend(vec![
        (its_event_authority, event_authority_account),
        (context.program_id, its_program_account),
    ]);

    // Execute the instruction
    let result = context.mollusk.process_and_validate_instruction(
        &execute_instruction,
        &execute_accounts,
        &checks,
    );

    ExecuteTestResult {
        result,
        instruction: execute_instruction,
        all_accounts: execute_accounts,
    }
}

/// Helper to create extra account metas for deploy interchain token
pub fn deploy_interchain_token_extra_accounts(
    deployer_ata: Pubkey,
    deployer: Pubkey,
    metadata_account: Pubkey,
) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
    solana_axelar_its::instructions::gmp::execute::execute_deploy_interchain_token_extra_accounts(
        deployer_ata,
        deployer,
        solana_sdk::sysvar::instructions::ID,
        mpl_token_metadata::ID,
        metadata_account,
        None,
        None,
    )
}

/// Helper to create extra account metas for link token
pub fn link_token_extra_accounts(
    deployer: Pubkey,
) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
    solana_axelar_its::instructions::gmp::execute::execute_link_token_extra_accounts(
        deployer, None, None,
    )
}

/// Helper to create extra account metas for interchain transfer
pub fn interchain_transfer_extra_accounts(
    destination_token_account: Pubkey,
    destination: Pubkey,
) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
    solana_axelar_its::instructions::gmp::execute::execute_interchain_transfer_extra_accounts(
        destination_token_account,
        destination,
    )
}
