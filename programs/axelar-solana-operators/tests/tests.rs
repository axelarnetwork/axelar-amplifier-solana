use anchor_lang::{AccountDeserialize, Key};
use axelar_solana_operators::{OperatorAccount, OperatorRegistry};
use mollusk_svm::{program::keyed_account_for_system_program, result::Check};
use solana_sdk::account::ReadableAccount;
use {
    anchor_lang::{
        solana_program::instruction::Instruction, system_program, Discriminator, InstructionData,
        Space, ToAccountMetas,
    },
    mollusk_svm::Mollusk,
    solana_sdk::{account::Account, pubkey::Pubkey},
};

// TODO(v2) extract to a common test utils crate
// or set the env var differently
pub(crate) fn setup_mollusk(program_id: &Pubkey, program_name: &str) -> Mollusk {
    std::env::set_var("SBF_OUT_DIR", "../../target/deploy");
    Mollusk::new(program_id, program_name)
}

fn setup_registry(
    mollusk: &Mollusk,
    program_id: Pubkey,
    master_operator: Pubkey,
    master_operator_account: &Account,
) -> (Pubkey, Account) {
    // Derive the registry PDA
    let (registry, _bump) = Pubkey::find_program_address(
        &[axelar_solana_operators::OperatorRegistry::SEED_PREFIX],
        &program_id,
    );

    // Initialize the registry instruction
    let ix1 = Instruction {
        program_id,
        accounts: axelar_solana_operators::accounts::Initialize {
            payer: master_operator,
            master_operator,
            registry,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
        data: axelar_solana_operators::instruction::Initialize {}.data(),
    };

    let accounts = vec![
        (master_operator, master_operator_account.clone()),
        (registry, Account::new(0, 0, &system_program::ID)),
        keyed_account_for_system_program(),
    ];

    let checks1 = vec![
        Check::success(),
        Check::account(&registry)
            .space(OperatorRegistry::DISCRIMINATOR.len() + OperatorRegistry::INIT_SPACE)
            .build(),
        Check::all_rent_exempt(),
    ];

    let result = mollusk.process_and_validate_instruction(&ix1, &accounts, &checks1);

    let registry_account = result
        .get_account(&registry)
        .expect("Registry account should exist");

    (registry, registry_account.clone())
}

pub fn add_operator(
    mollusk: &Mollusk,
    program_id: Pubkey,
    (registry, registry_account): (Pubkey, Account),
    (master_operator, master_operator_account): (Pubkey, Account),
    (operator_to_add, operator_to_add_account): (Pubkey, Account),
) -> (Account, Pubkey, Account) {
    // Derive the operator PDA
    let (operator_to_add_pda, _bump) = Pubkey::find_program_address(
        &[
            axelar_solana_operators::OperatorAccount::SEED_PREFIX,
            operator_to_add.key().as_ref(),
        ],
        &program_id,
    );

    // Add operator instruction
    let ix = Instruction {
        program_id,
        accounts: axelar_solana_operators::accounts::AddOperator {
            master_operator,
            operator_to_add,
            registry,
            operator_account: operator_to_add_pda,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
        data: axelar_solana_operators::instruction::AddOperator {}.data(),
    };

    let checks = vec![
        Check::success(),
        Check::account(&operator_to_add_pda)
            .space(OperatorAccount::DISCRIMINATOR.len() + OperatorAccount::INIT_SPACE)
            .build(),
        Check::all_rent_exempt(),
    ];

    // List accounts
    let accounts = vec![
        (operator_to_add, operator_to_add_account.clone()),
        (master_operator, master_operator_account),
        (registry, registry_account),
        (operator_to_add_pda, Account::new(0, 0, &system_program::ID)),
        keyed_account_for_system_program(),
    ];

    let result = mollusk.process_and_validate_instruction(&ix, &accounts, &checks);

    let operator_pda_account = result
        .get_account(&operator_to_add_pda)
        .expect("Operator PDA should exist");

    // Return the updated registry account
    let registry_account = result
        .get_account(&registry)
        .expect("Registry account should exist")
        .clone();

    (
        registry_account,
        operator_to_add_pda,
        operator_pda_account.clone(),
    )
}

pub fn remove_operator(
    mollusk: &Mollusk,
    program_id: Pubkey,
    (registry, registry_account): (Pubkey, Account),
    (master_operator, master_operator_account): (Pubkey, Account),
    (operator_to_remove, operator_to_remove_account): (Pubkey, Account),
    (operator_pda, operator_pda_account): (Pubkey, Account),
) -> Account {
    // Remove operator instruction
    let ix = Instruction {
        program_id,
        accounts: axelar_solana_operators::accounts::RemoveOperator {
            master_operator,
            operator_to_remove,
            registry,
            operator_account: operator_pda,
        }
        .to_account_metas(None),
        data: axelar_solana_operators::instruction::RemoveOperator {}.data(),
    };

    // The operator account should be closed and its lamports returned to master_operator
    let initial_master_balance = master_operator_account.lamports;
    let operator_account_lamports = operator_pda_account.lamports;

    let checks = vec![
        Check::success(),
        // Operator account should be closed (no longer exist)
        Check::account(&operator_pda).closed().build(),
        // Master operator should receive the lamports from the closed account
        Check::account(&master_operator)
            .lamports(initial_master_balance + operator_account_lamports)
            .build(),
    ];

    let accounts = vec![
        (master_operator, master_operator_account),
        (operator_to_remove, operator_to_remove_account),
        (registry, registry_account.clone()),
        (operator_pda, operator_pda_account),
    ];

    let result = mollusk.process_and_validate_instruction(&ix, &accounts, &checks);

    // Return the updated registry account
    result
        .get_account(&registry)
        .expect("Registry account should exist")
        .clone()
}

pub fn transfer_master(
    mollusk: &Mollusk,
    program_id: Pubkey,
    (registry, registry_account): (Pubkey, Account),
    (current_master, current_master_account): (Pubkey, Account),
    (new_master, new_master_account): (Pubkey, Account),
) -> Account {
    // Transfer master instruction
    let ix = Instruction {
        program_id,
        accounts: axelar_solana_operators::accounts::TransferMaster {
            current_master,
            new_master,
            registry,
        }
        .to_account_metas(None),
        data: axelar_solana_operators::instruction::TransferMaster {}.data(),
    };

    let checks = vec![
        Check::success(),
        Check::account(&registry)
            .data_slice(OperatorRegistry::DISCRIMINATOR.len(), new_master.as_array())
            .build(),
    ];

    let accounts = vec![
        (current_master, current_master_account),
        (new_master, new_master_account),
        (registry, registry_account.clone()),
    ];

    let result = mollusk.process_and_validate_instruction(&ix, &accounts, &checks);

    // Return the updated registry account
    result
        .get_account(&registry)
        .expect("Registry account should exist")
        .clone()
}

#[test]
fn test_initialize_add_remove() {
    let program_id = axelar_solana_operators::id();

    let mollusk = setup_mollusk(&program_id, "axelar_solana_operators");

    let master_operator = Pubkey::new_unique();
    let master_operator_account = Account::new(1_000_000_000, 0, &system_program::ID);

    let (registry, registry_account) = setup_registry(
        &mollusk,
        program_id,
        master_operator,
        &master_operator_account,
    );

    // Add first operator
    let operator1 = Pubkey::new_unique();
    let operator1_account = Account::new(1_000_000_000, 0, &system_program::ID);

    let (registry_account, operator1_pda, operator1_pda_account) = add_operator(
        &mollusk,
        program_id,
        (registry, registry_account.clone()),
        (master_operator, master_operator_account.clone()),
        (operator1, operator1_account.clone()),
    );

    // Add second operator
    let operator2 = Pubkey::new_unique();
    let operator2_account = Account::new(1_000_000_000, 0, &system_program::ID);

    let (registry_account, _, _) = add_operator(
        &mollusk,
        program_id,
        (registry, registry_account.clone()),
        (master_operator, master_operator_account.clone()),
        (operator2, operator2_account),
    );

    // Remove the first operator
    let registry_account = remove_operator(
        &mollusk,
        program_id,
        (registry, registry_account.clone()),
        (master_operator, master_operator_account.clone()),
        (operator1, operator1_account),
        (operator1_pda, operator1_pda_account),
    );

    let registry_state: OperatorRegistry =
        OperatorRegistry::try_deserialize(&mut registry_account.data())
            .expect("Failed to deserialize registry account");

    assert_eq!(
        registry_state.operator_count, 1,
        "Operator count should be decremented to 1"
    );
}

#[test]
fn test_transfer_master() {
    let program_id = axelar_solana_operators::id();
    let mollusk = setup_mollusk(&program_id, "axelar_solana_operators");

    let original_master = Pubkey::new_unique();
    let original_master_account = Account::new(1_000_000_000, 0, &system_program::ID);

    // Setup registry with original master
    let (registry, registry_account) = setup_registry(
        &mollusk,
        program_id,
        original_master,
        &original_master_account,
    );

    // Create new master operator
    let new_master = Pubkey::new_unique();
    let new_master_account = Account::new(1_000_000_000, 0, &system_program::ID);

    // Transfer master operatorship
    let updated_registry_account = transfer_master(
        &mollusk,
        program_id,
        (registry, registry_account),
        (original_master, original_master_account.clone()),
        (new_master, new_master_account.clone()),
    );

    // Verify the master operator has been updated
    let registry_state: OperatorRegistry =
        OperatorRegistry::try_deserialize(&mut updated_registry_account.data())
            .expect("Failed to deserialize registry account");

    assert_eq!(
        registry_state.master_operator, new_master,
        "Master operator should be updated to new master"
    );

    // Test that the new master can now add operators
    let operator = Pubkey::new_unique();
    let operator_account = Account::new(1_000_000_000, 0, &system_program::ID);

    let (_registry_account, _operator_pda, _operator_pda_account) = add_operator(
        &mollusk,
        program_id,
        (registry, updated_registry_account),
        (new_master, new_master_account), // New master should be able to add operators
        (operator, operator_account),
    );
}
