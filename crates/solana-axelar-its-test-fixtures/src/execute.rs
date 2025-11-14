use crate::get_message_signing_pda;
use anchor_lang::{prelude::AccountMeta, InstructionData, ToAccountMetas};
use mollusk_svm::result::{Check, InstructionResult};
use mollusk_svm::Mollusk;
use mollusk_test_utils::get_event_authority_and_program_accounts;
use solana_axelar_gateway::{Message, ID as GATEWAY_PROGRAM_ID};
use solana_axelar_its::state::TokenManager;
use solana_sdk::{account::Account, instruction::Instruction, pubkey::Pubkey};

pub struct ExecuteTestContext {
    pub mollusk: Mollusk,
    pub gateway_root: (Pubkey, Account),
    pub its_root: (Pubkey, Account),
    pub payer: (Pubkey, Account),
}

pub struct ExecuteTestParams {
    pub message: Message,
    pub payload: Vec<u8>,
    pub token_id: [u8; 32],
    pub incoming_message_pda: Pubkey,
    pub incoming_message_account_data: Vec<u8>,
}

pub struct ExecuteTestAccounts {
    /// Core ITS accounts that are always needed
    pub core_accounts: Vec<(Pubkey, Account)>,
    /// Extra accounts specific to the instruction type (e.g., for deploy, link, transfer)
    pub extra_accounts: Vec<(Pubkey, Account)>,
    /// Extra account metas for remaining accounts
    pub extra_account_metas: Vec<AccountMeta>,
    /// Optional token manager account data (if None, creates empty account)
    pub token_manager_account: Option<Account>,
}

pub struct ExecuteTestResult {
    pub result: InstructionResult,
    pub instruction: Instruction,
    pub all_accounts: Vec<(Pubkey, Account)>,
}

#[allow(clippy::indexing_slicing)]
pub fn execute_its_instruction(
    context: ExecuteTestContext,
    params: ExecuteTestParams,
    accounts_config: ExecuteTestAccounts,
    checks: Vec<Check>,
) -> ExecuteTestResult {
    let program_id = solana_axelar_its::id();

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
        token_manager_account,
    } = accounts_config;

    let (token_manager_pda, _) = TokenManager::find_pda(token_id, context.its_root.0);

    let (signing_pda, _) = get_message_signing_pda(&message);

    let (gateway_event_authority, _, _) =
        get_event_authority_and_program_accounts(&GATEWAY_PROGRAM_ID);

    let (its_event_authority, event_authority_account, its_program_account) =
        get_event_authority_and_program_accounts(&program_id);

    let instruction_data = solana_axelar_its::instruction::Execute {
        message: message.clone(),
        payload,
    };

    // Create base accounts
    let executable_accounts = solana_axelar_its::accounts::AxelarExecuteAccounts {
        incoming_message_pda,
        signing_pda,
        gateway_root_pda: context.gateway_root.0,
        event_authority: gateway_event_authority,
        axelar_gateway_program: GATEWAY_PROGRAM_ID,
    };

    let base_accounts = solana_axelar_its::accounts::Execute {
        executable: executable_accounts,
        payer: context.payer.0,
        its_root_pda: context.its_root.0,
        token_manager_pda,
        token_mint: core_accounts[0].0, // First core account should be token_mint
        token_manager_ata: core_accounts[1].0, // Second should be token_manager_ata
        token_program: anchor_spl::token_2022::spl_token_2022::id(),
        associated_token_program: anchor_spl::associated_token::spl_associated_token_account::id(),
        system_program: solana_sdk::system_program::ID,
        event_authority: its_event_authority,
        program: program_id,
    };

    let mut account_metas = base_accounts.to_account_metas(None);
    account_metas.extend(extra_account_metas);

    let execute_instruction = Instruction {
        program_id,
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
        (context.gateway_root.0, context.gateway_root.1),
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
        (context.payer.0, context.payer.1),
        (context.its_root.0, context.its_root.1),
        // empty: linkToken, deployInterchainToken since its deployed
        // non-empty: interchainTransfer
        (
            token_manager_pda,
            token_manager_account
                .unwrap_or_else(|| Account::new(0, 0, &solana_sdk::system_program::ID)),
        ),
    ];

    // Add core accounts (token_mint, token_manager_ata, etc.)
    execute_accounts.extend(core_accounts);

    // Add common system accounts
    execute_accounts.extend(vec![
        mollusk_svm_programs_token::token2022::keyed_account(),
        mollusk_svm_programs_token::associated_token::keyed_account(),
        mollusk_svm::program::keyed_account_for_system_program(),
    ]);

    // Add extra accounts
    execute_accounts.extend(extra_accounts);

    // Add event CPI accounts
    execute_accounts.extend(vec![
        (its_event_authority, event_authority_account),
        (program_id, its_program_account),
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
        destination,
        destination_token_account,
    )
}
