use anchor_lang::{InstructionData, ToAccountMetas};
use mollusk_svm::program::keyed_account_for_system_program;
use mollusk_svm::result::Check;
use mollusk_svm::Mollusk;
use mollusk_test_utils::get_event_authority_and_program_accounts;
use solana_axelar_its::state::token_manager::Type;
use solana_sdk::{account::Account, instruction::Instruction, pubkey::Pubkey};

pub struct RegisterCustomTokenContext {
    pub mollusk: Mollusk,
    pub payer: (Pubkey, Account),
    pub deployer: (Pubkey, Account),
    pub its_root: (Pubkey, Account),
    pub token_mint: (Pubkey, Account),
}

pub struct RegisterCustomTokenParams {
    pub salt: [u8; 32],
    pub token_manager_type: Type,
    pub operator: Option<Pubkey>,
}

pub struct RegisterCustomTokenResult {
    pub instruction: Instruction,
    pub accounts: Vec<(Pubkey, Account)>,
    pub result: mollusk_svm::result::InstructionResult,
    pub token_manager_pda: Pubkey,
    pub token_manager_ata: Pubkey,
    pub mollusk: Mollusk,
}

pub fn execute_register_custom_token_helper(
    ctx: RegisterCustomTokenContext,
    params: RegisterCustomTokenParams,
    checks: Vec<Check>,
) -> RegisterCustomTokenResult {
    let program_id = solana_axelar_its::id();

    // Calculate token ID and derive PDAs
    let token_id = {
        let deploy_salt =
            solana_axelar_its::utils::linked_token_deployer_salt(&ctx.deployer.0, &params.salt);
        solana_axelar_its::utils::interchain_token_id_internal(&deploy_salt)
    };

    let (token_manager_pda, _) =
        solana_axelar_its::state::TokenManager::find_pda(token_id, ctx.its_root.0);

    let token_manager_ata =
        anchor_spl::associated_token::get_associated_token_address_with_program_id(
            &token_manager_pda,
            &ctx.token_mint.0,
            &anchor_spl::token_2022::spl_token_2022::ID,
        );

    let (event_authority, _, _) = get_event_authority_and_program_accounts(&program_id);

    // Handle operator and operator roles PDA
    let (operator_account, operator_roles_pda) = if let Some(operator) = params.operator {
        let (operator_roles_pda, _) =
            solana_axelar_its::state::UserRoles::find_pda(&token_manager_pda, &operator);
        (Some(operator), Some(operator_roles_pda))
    } else {
        (None, None)
    };

    // Create the instruction data
    let instruction_data = solana_axelar_its::instruction::RegisterCustomToken {
        salt: params.salt,
        token_manager_type: params.token_manager_type,
        operator: params.operator,
    };

    // Build account metas
    let accounts = solana_axelar_its::accounts::RegisterCustomToken {
        payer: ctx.payer.0,
        deployer: ctx.deployer.0,
        system_program: solana_sdk_ids::system_program::ID,
        its_root_pda: ctx.its_root.0,
        token_manager_pda,
        token_mint: ctx.token_mint.0,
        token_manager_ata,
        token_program: anchor_spl::token_2022::spl_token_2022::ID,
        associated_token_program: anchor_spl::associated_token::ID,
        operator: operator_account,
        operator_roles_pda,
        // for event cpi
        event_authority,
        program: program_id,
    };

    let ix = Instruction {
        program_id,
        accounts: accounts.to_account_metas(None),
        data: instruction_data.data(),
    };

    // Setup accounts for mollusk
    let mut mollusk_accounts = vec![
        (ctx.payer.0, ctx.payer.1),
        (ctx.deployer.0, ctx.deployer.1),
        keyed_account_for_system_program(),
        (ctx.its_root.0, ctx.its_root.1),
        (
            token_manager_pda,
            Account::new(0, 0, &solana_sdk_ids::system_program::ID),
        ),
        (ctx.token_mint.0, ctx.token_mint.1),
        (
            token_manager_ata,
            Account::new(0, 0, &solana_sdk_ids::system_program::ID),
        ),
        mollusk_svm_programs_token::token2022::keyed_account(),
        mollusk_svm_programs_token::associated_token::keyed_account(),
        (
            anchor_spl::associated_token::ID,
            Account {
                lamports: 1,
                data: vec![],
                owner: solana_sdk_ids::bpf_loader_upgradeable::ID,
                executable: true,
                rent_epoch: 0,
            },
        ),
        (
            solana_sdk::sysvar::rent::ID,
            Account {
                lamports: 1_000_000_000,
                data: {
                    let rent = anchor_lang::prelude::Rent::default();
                    bincode::serialize(&rent).unwrap()
                },
                owner: solana_sdk::sysvar::rent::ID,
                executable: false,
                rent_epoch: 0,
            },
        ),
        // For event CPI
        (
            event_authority,
            Account::new(0, 0, &solana_sdk_ids::system_program::ID),
        ),
        (
            program_id,
            Account::new(0, 0, &solana_sdk_ids::system_program::ID),
        ),
    ];

    // Add operator account if provided
    if let Some(operator) = params.operator {
        mollusk_accounts.push((
            operator,
            Account::new(0, 0, &solana_sdk_ids::system_program::ID),
        ));
    }

    // Add operator roles PDA if provided
    if let Some(operator_roles_pda) = operator_roles_pda {
        mollusk_accounts.push((
            operator_roles_pda,
            Account::new(0, 0, &solana_sdk_ids::system_program::ID),
        ));
    }

    // Execute instruction
    let result = ctx
        .mollusk
        .process_and_validate_instruction(&ix, &mollusk_accounts, &checks);

    RegisterCustomTokenResult {
        instruction: ix,
        accounts: mollusk_accounts,
        result,
        token_manager_pda,
        token_manager_ata,
        mollusk: ctx.mollusk,
    }
}
