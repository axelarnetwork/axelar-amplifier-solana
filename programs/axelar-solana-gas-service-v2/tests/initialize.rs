// #![cfg(feature = "test-sbf")]

use anchor_lang::Key;
use axelar_solana_gas_service_v2::state::Treasury;
use mollusk_svm::{program::keyed_account_for_system_program, result::Check};
use {
    anchor_lang::{
        solana_program::instruction::Instruction, system_program, Discriminator, InstructionData,
        Space, ToAccountMetas,
    },
    mollusk_svm::Mollusk,
    solana_sdk::{account::Account, pubkey::Pubkey},
    solana_sdk_ids::bpf_loader_upgradeable,
};

// TODO(v2) extract to a common test utils crate
// or set the env var differently
pub fn setup_mollusk(program_id: &Pubkey, program_name: &str) -> Mollusk {
    std::env::set_var("SBF_OUT_DIR", "../../target/deploy");
    Mollusk::new(program_id, program_name)
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
        &bpf_loader_upgradeable::ID,
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
            operator.key().as_ref(),
        ],
        &program_id,
    );

    // Initialize the registry instruction
    let ix1 = Instruction {
        program_id,
        accounts: axelar_solana_operators::accounts::Initialize {
            payer: operator,
            master_operator: operator,
            registry,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
        data: axelar_solana_operators::instruction::Initialize {}.data(),
    };

    // Add operator instruction
    let ix2 = Instruction {
        program_id,
        accounts: axelar_solana_operators::accounts::AddOperator {
            master_operator: operator,
            operator_to_add: operator,
            registry,
            operator_account: operator_pda,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
        data: axelar_solana_operators::instruction::AddOperator {}.data(),
    };

    // List accounts
    let accounts = vec![
        (operator, operator_account.clone()),
        (registry, Account::new(0, 0, &system_program::ID)),
        (operator_pda, Account::new(0, 0, &system_program::ID)),
        keyed_account_for_system_program(),
    ];

    let result = mollusk.process_instruction_chain(&[ix1, ix2], &accounts);
    assert!(result.program_result.is_ok());

    let operator_pda_account = result
        .get_account(&operator_pda)
        .expect("Operator PDA should exist");

    (operator_pda, operator_pda_account.clone())
}

fn init_gas_service(
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
            system_program: system_program::ID,
        }
        .to_account_metas(None),
        data: axelar_solana_gas_service_v2::instruction::Initialize {}.data(),
    };

    let accounts = vec![
        (operator, operator_account.clone()),
        (operator_pda, operator_pda_account.clone()),
        (treasury, Account::new(0, 0, &system_program::ID)),
        keyed_account_for_system_program(),
    ];

    let result = mollusk.process_instruction(&ix, &accounts);
    assert!(result.program_result.is_ok(), "should initialize");

    let treasury_pda = result
        .get_account(&treasury)
        .expect("Treasury PDA should exist");

    let expected_size = Treasury::DISCRIMINATOR.len() + Treasury::INIT_SPACE;
    assert_eq!(treasury_pda.data.len(), expected_size);

    (treasury, treasury_pda.clone())
}

// TODO(v2) improve tests and use mollusk checks for more precise assertions

#[test]
fn test_initialize_success() {
    let program_id = axelar_solana_gas_service_v2::id();
    let mut mollusk = setup_mollusk(&program_id, "axelar_solana_gas_service_v2");

    let operator = Pubkey::new_unique();
    let operator_account = Account::new(1_000_000_000, 0, &system_program::ID);

    let (operator_pda, operator_pda_account) =
        setup_operator(&mut mollusk, operator, &operator_account);

    let (_treasury, _treasury_pda) = init_gas_service(
        &mollusk,
        operator,
        &operator_account,
        operator_pda,
        &operator_pda_account,
    );
}

#[test]
#[should_panic(expected = "should initialize")]
fn test_initialize_unauthorized() {
    let program_id = axelar_solana_gas_service_v2::id();
    let mut mollusk = setup_mollusk(&program_id, "axelar_solana_gas_service_v2");

    let operator = Pubkey::new_unique();
    let operator_account = Account::new(1_000_000_000, 0, &system_program::ID);

    let unauthorized_operator = Pubkey::new_unique();
    let unauthorized_operator_account = Account::new(1_000_000_000, 0, &system_program::ID);

    let (operator_pda, operator_pda_account) =
        setup_operator(&mut mollusk, operator, &operator_account);

    let (_treasury, _treasury_pda) = init_gas_service(
        &mollusk,
        unauthorized_operator,
        &unauthorized_operator_account,
        operator_pda,
        &operator_pda_account,
    );
}

#[test]
fn test_add_native_gas() {
    // Setup

    let program_id = axelar_solana_gas_service_v2::id();
    let mut mollusk = setup_mollusk(&program_id, "axelar_solana_gas_service_v2");

    let operator = Pubkey::new_unique();
    let operator_account = Account::new(1_000_000_000, 0, &system_program::ID);

    let (operator_pda, operator_pda_account) =
        setup_operator(&mut mollusk, operator, &operator_account);

    let (treasury, treasury_pda) = init_gas_service(
        &mollusk,
        operator,
        &operator_account,
        operator_pda,
        &operator_pda_account,
    );

    // Instruction

    let sender = Pubkey::new_unique();
    let sender_balance = 1_000_000_000u64; // 1 SOL
    let sender_account = Account::new(sender_balance, 0, &system_program::ID);

    let tx_hash = [0u8; 64];
    let log_index = 0u64;
    let gas_fee_amount = 300_000_000u64; // 0.3 SOL
    let refund_address = Pubkey::new_unique();

    let (event_authority, _bump) =
        Pubkey::find_program_address(&[b"__event_authority"], &program_id);
    let event_authority_account = Account::new(0, 0, &system_program::ID);

    let ix = Instruction {
        program_id,
        accounts: axelar_solana_gas_service_v2::accounts::AddNativeGas {
            sender,
            treasury,
            system_program: system_program::ID,
            // Event authority
            event_authority: event_authority,
            // The current program account
            program: program_id,
        }
        .to_account_metas(None),
        data: axelar_solana_gas_service_v2::instruction::AddNativeGas {
            tx_hash,
            log_index,
            gas_fee_amount,
            refund_address,
        }
        .data(),
    };

    let accounts = vec![
        (sender, sender_account.clone()),
        (treasury, treasury_pda.clone()),
        keyed_account_for_system_program(),
        // Event authority
        (event_authority, event_authority_account),
        // Current program account (executable)
        (
            program_id,
            Account {
                lamports: 1,
                data: vec![],
                owner: bpf_loader_upgradeable::ID,
                executable: true,
                rent_epoch: 0,
            },
        ),
    ];

    // Checks

    let checks = vec![
        Check::success(),
        // Balance subtracted
        Check::account(&sender)
            .lamports(sender_balance - gas_fee_amount)
            .build(),
        // Balance added
        Check::account(&treasury)
            .lamports(treasury_pda.lamports + gas_fee_amount)
            .build(),
    ];

    let result = mollusk.process_and_validate_instruction(&ix, &accounts, &checks);
    assert!(result.program_result.is_ok(), "should add native gas");

    // TODO(v2) check for CPI event emission
}
