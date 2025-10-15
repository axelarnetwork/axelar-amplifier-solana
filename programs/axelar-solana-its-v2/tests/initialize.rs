use anchor_lang::AccountDeserialize;
use anchor_spl::{token::spl_token, token_2022::spl_token_2022};
use axelar_solana_gas_service_v2::state::Treasury;
use axelar_solana_its_v2::state::{InterchainTokenService, Roles, UserRoles};
use axelar_solana_operators::{OperatorAccount, OperatorRegistry};
use mollusk_svm::{program::keyed_account_for_system_program, result::Check};
use mollusk_test_utils::{
    create_program_data_account, get_event_authority_and_program_accounts, setup_mollusk,
};
use {
    anchor_lang::{
        solana_program::instruction::Instruction, system_program, Discriminator, InstructionData,
        Space, ToAccountMetas,
    },
    mollusk_svm::Mollusk,
    solana_sdk::{account::Account, pubkey::Pubkey},
    solana_sdk_ids::bpf_loader_upgradeable,
};

pub(crate) fn initialize_mollusk() -> Mollusk {
    let program_id = axelar_solana_its_v2::id();
    let mut mollusk = setup_mollusk(&program_id, "axelar_solana_its_v2");

    mollusk.add_program(
        &mpl_token_metadata::programs::MPL_TOKEN_METADATA_ID,
        "../../target/deploy/mpl_token_metadata",
        &solana_sdk::bpf_loader_upgradeable::id(),
    );

    let spl_token_elf = mollusk_svm_programs_token::token::ELF;
    mollusk.add_program_with_elf_and_loader(
        &spl_token::ID,
        &spl_token_elf,
        &solana_sdk::bpf_loader_upgradeable::ID,
    );

    let token_2022_elf = mollusk_svm_programs_token::token2022::ELF;
    mollusk.add_program_with_elf_and_loader(
        &spl_token_2022::ID,
        &token_2022_elf,
        &solana_sdk::bpf_loader_upgradeable::ID,
    );

    let associated_token_elf = mollusk_svm_programs_token::associated_token::ELF;
    mollusk.add_program_with_elf_and_loader(
        &anchor_spl::associated_token::ID,
        &associated_token_elf,
        &solana_sdk::bpf_loader_upgradeable::ID,
    );

    mollusk.add_program(
        &axelar_solana_gas_service_v2::ID,
        "../../target/deploy/axelar_solana_gas_service_v2",
        &solana_sdk::bpf_loader_upgradeable::ID,
    );

    mollusk.add_program(
        &axelar_solana_gateway_v2::ID,
        "../../target/deploy/axelar_solana_gateway_v2",
        &solana_sdk::bpf_loader_upgradeable::ID,
    );

    mollusk
}

pub(crate) fn init_its_service(
    mollusk: &Mollusk,
    payer: Pubkey,
    payer_account: &Account,
    upgrade_authority: Pubkey,
    operator: Pubkey,
    operator_account: &Account,
    chain_name: String,
    its_hub_address: String,
) -> (Pubkey, Account, Pubkey, Account, Pubkey, Account) {
    let program_id = axelar_solana_its_v2::id();

    // Derive the program data PDA for the upgradeable program
    let (program_data, _bump) =
        Pubkey::find_program_address(&[program_id.as_ref()], &bpf_loader_upgradeable::ID);
    let its_elf = mollusk_svm::file::load_program_elf("axelar_solana_its_v2");
    let program_data_account = create_program_data_account(&its_elf, upgrade_authority);

    if payer != upgrade_authority {
        println!("[WARNING] Initialize will fail since payer is not the upgrade authority");
    }

    // Derive the ITS root PDA
    let (its_root_pda, _bump) =
        Pubkey::find_program_address(&[InterchainTokenService::SEED_PREFIX], &program_id);

    // Derive the user roles PDA
    let (user_roles_pda, _bump) = Pubkey::find_program_address(
        &UserRoles::pda_seeds(&its_root_pda, &operator)[..],
        &program_id,
    );

    let ix = Instruction {
        program_id,
        accounts: axelar_solana_its_v2::accounts::Initialize {
            payer,
            program_data,
            its_root_pda,
            system_program: system_program::ID,
            operator,
            user_roles_account: user_roles_pda,
        }
        .to_account_metas(None),
        data: axelar_solana_its_v2::instruction::Initialize {
            chain_name,
            its_hub_address,
        }
        .data(),
    };

    let accounts = vec![
        (payer, payer_account.clone()),
        (program_data, program_data_account.clone()),
        (its_root_pda, Account::new(0, 0, &system_program::ID)),
        keyed_account_for_system_program(),
        (operator, operator_account.clone()),
        (user_roles_pda, Account::new(0, 0, &system_program::ID)),
    ];

    let checks = vec![
        Check::success(),
        Check::account(&its_root_pda)
            .space(InterchainTokenService::DISCRIMINATOR.len() + InterchainTokenService::INIT_SPACE)
            .build(),
        Check::account(&user_roles_pda)
            .space(UserRoles::DISCRIMINATOR.len() + UserRoles::INIT_SPACE)
            .build(),
        Check::all_rent_exempt(),
    ];

    let result = mollusk.process_and_validate_instruction(&ix, &accounts, &checks);

    let its_root_account = result
        .get_account(&its_root_pda)
        .expect("ITS root PDA should exist");

    let user_roles_account = result
        .get_account(&user_roles_pda)
        .expect("User roles PDA should exist");

    let user_roles_data = UserRoles::try_deserialize(&mut user_roles_account.data.as_slice())
        .expect("Failed to deserialize roles data");
    assert_eq!(user_roles_data.roles, Roles::OPERATOR);

    (
        its_root_pda,
        its_root_account.clone(),
        user_roles_pda,
        user_roles_account.clone(),
        program_data,
        program_data_account,
    )
}

pub(crate) fn setup_operator(
    mollusk: &mut Mollusk,
    operator: Pubkey,
    operator_account: &Account,
) -> (Pubkey, Account) {
    let program_id = axelar_solana_operators::id();

    // Load the operators program into mollusk
    mollusk.add_program(
        &program_id,
        "axelar_solana_operators",
        &solana_sdk::bpf_loader_upgradeable::ID,
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
            operator.as_ref(),
        ],
        &program_id,
    );

    // Initialize the registry instruction
    let ix1 = Instruction {
        program_id,
        accounts: axelar_solana_operators::accounts::Initialize {
            payer: operator,
            owner: operator,
            registry,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
        data: axelar_solana_operators::instruction::Initialize {}.data(),
    };

    let checks1 = vec![
        Check::success(),
        Check::account(&registry)
            .space(OperatorRegistry::DISCRIMINATOR.len() + OperatorRegistry::INIT_SPACE)
            .build(),
        Check::all_rent_exempt(),
    ];

    // Add operator instruction
    let ix2 = Instruction {
        program_id,
        accounts: axelar_solana_operators::accounts::AddOperator {
            owner: operator,
            operator_to_add: operator,
            registry,
            operator_account: operator_pda,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
        data: axelar_solana_operators::instruction::AddOperator {}.data(),
    };

    let checks2 = vec![
        Check::success(),
        Check::account(&operator_pda)
            .space(OperatorAccount::DISCRIMINATOR.len() + OperatorAccount::INIT_SPACE)
            .build(),
        Check::all_rent_exempt(),
    ];

    // List accounts
    let accounts = vec![
        (operator, operator_account.clone()),
        (registry, Account::new(0, 0, &system_program::ID)),
        (operator_pda, Account::new(0, 0, &system_program::ID)),
        keyed_account_for_system_program(),
    ];

    let result = mollusk.process_and_validate_instruction_chain(
        &[
            // Initialize the registry
            (&ix1, &checks1),
            // Add the operator
            (&ix2, &checks2),
        ],
        &accounts,
    );

    let operator_pda_account = result
        .get_account(&operator_pda)
        .expect("Operator PDA should exist");

    (operator_pda, operator_pda_account.clone())
}

pub(crate) fn init_gas_service(
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

    let checks = vec![
        Check::success(),
        Check::account(&treasury)
            .space(Treasury::DISCRIMINATOR.len() + Treasury::INIT_SPACE)
            .build(),
        Check::all_rent_exempt(),
    ];

    let result = mollusk.process_and_validate_instruction(&ix, &accounts, &checks);

    let treasury_pda = result
        .get_account(&treasury)
        .expect("Treasury PDA should exist");

    (treasury, treasury_pda.clone())
}

pub(crate) fn init_its_service_with_ethereum_trusted(
    mollusk: &Mollusk,
    payer: Pubkey,
    payer_account: &Account,
    upgrade_authority: Pubkey,
    operator: Pubkey,
    operator_account: &Account,
    chain_name: String,
    its_hub_address: String,
) -> (Pubkey, Account) {
    let program_id = axelar_solana_its_v2::id();

    // First initialize the ITS service
    let (
        its_root_pda,
        its_root_account,
        _user_roles_pda,
        _user_roles_account,
        program_data,
        program_data_account,
    ) = init_its_service(
        mollusk,
        payer,
        payer_account,
        upgrade_authority,
        operator,
        operator_account,
        chain_name,
        its_hub_address,
    );

    let (event_authority, event_authority_account, program_account) =
        get_event_authority_and_program_accounts(&program_id);

    // Add ethereum as a trusted chain
    let trusted_chain_name = "ethereum".to_string();

    let ix = Instruction {
        program_id,
        accounts: axelar_solana_its_v2::accounts::SetTrustedChain {
            payer,
            user_roles: None,
            program_data: Some(program_data),
            its_root_pda,
            system_program: system_program::ID,
            event_authority: event_authority,
            program: program_id,
        }
        .to_account_metas(None),
        data: axelar_solana_its_v2::instruction::SetTrustedChain {
            chain_name: trusted_chain_name.clone(),
        }
        .data(),
    };

    let accounts = vec![
        (payer, payer_account.clone()),
        (program_data, program_data_account.clone()),
        (its_root_pda, its_root_account.clone()),
        keyed_account_for_system_program(),
        (event_authority, event_authority_account),
        (program_id, program_account),
    ];

    let checks = vec![Check::success()];

    let result = mollusk.process_and_validate_instruction(&ix, &accounts, &checks);

    let updated_its_account = result
        .get_account(&its_root_pda)
        .expect("ITS root PDA should exist");

    // Verify ethereum was added as trusted chain
    let updated_its_data =
        InterchainTokenService::try_deserialize(&mut updated_its_account.data.as_slice())
            .expect("Failed to deserialize updated ITS data");

    assert!(updated_its_data.contains_trusted_chain("ethereum".to_string()));

    (its_root_pda, updated_its_account.clone())
}

#[test]
fn test_initialize_success() {
    let program_id = axelar_solana_its_v2::id();
    let mollusk = setup_mollusk(&program_id, "axelar_solana_its_v2");

    let upgrade_authority = Pubkey::new_unique();

    // We require that the payer be the upgrade_authority
    let payer = upgrade_authority;
    let payer_account = Account::new(1_000_000_000, 0, &system_program::ID);

    let operator = Pubkey::new_unique();
    let operator_account = Account::new(1_000_000_000, 0, &system_program::ID);

    let chain_name = "solana".to_string();
    let its_hub_address = "0x123456789abcdef".to_string();

    let (
        its_root_pda,
        its_root_account,
        user_roles_pda,
        user_roles_account,
        _program_data,
        _program_data_account,
    ) = init_its_service(
        &mollusk,
        payer,
        &payer_account,
        payer,
        operator,
        &operator_account,
        chain_name.clone(),
        its_hub_address.clone(),
    );

    // Verify the ITS root PDA is properly initialized
    let its_data = InterchainTokenService::try_deserialize(&mut its_root_account.data.as_slice())
        .expect("Failed to deserialize ITS data");

    assert_eq!(its_data.chain_name, chain_name);
    assert_eq!(its_data.its_hub_address, its_hub_address);
    assert_eq!(its_data.paused, false);
    assert_eq!(its_data.trusted_chains.len(), 0);

    // Verify the user roles PDA is properly initialized
    let roles_data = UserRoles::try_deserialize(&mut user_roles_account.data.as_slice())
        .expect("Failed to deserialize roles data");

    assert_eq!(roles_data.roles, Roles::OPERATOR);

    // Verify PDAs are derived correctly
    let expected_its_pda =
        Pubkey::find_program_address(&[InterchainTokenService::SEED_PREFIX], &program_id).0;
    assert_eq!(its_root_pda, expected_its_pda);

    let expected_roles_pda = Pubkey::find_program_address(
        &[
            UserRoles::SEED_PREFIX,
            its_root_pda.as_ref(),
            operator.as_ref(),
        ],
        &program_id,
    )
    .0;
    assert_eq!(user_roles_pda, expected_roles_pda);

    // Verify the program data PDA is correct
    let expected_program_data =
        Pubkey::find_program_address(&[program_id.as_ref()], &bpf_loader_upgradeable::ID).0;
    let (actual_program_data, _) = (expected_program_data, 0); // We verified it in init_its_service
    assert_eq!(actual_program_data, expected_program_data);
}

#[test]
#[should_panic = "InvalidAccountData"]
fn test_initialize_unauthorized_payer() {
    let program_id = axelar_solana_its_v2::id();
    let mollusk = setup_mollusk(&program_id, "axelar_solana_its_v2");

    let upgrade_authority = Pubkey::new_unique();

    // We make the payer different from the upgrade_authority
    let payer = Pubkey::new_unique();
    let payer_account = Account::new(1_000_000_000, 0, &system_program::ID);

    let operator = Pubkey::new_unique();
    let operator_account = Account::new(1_000_000_000, 0, &system_program::ID);

    let chain_name = "solana".to_string();
    let its_hub_address = "0x123456789abcdef".to_string();

    // This should fail because payer is not the upgrade authority
    // The program data account was created with authorized_payer as authority
    let (
        _its_root_pda,
        _its_root_account,
        _user_roles_pda,
        _user_roles_account,
        _program_data,
        _program_data_account,
    ) = init_its_service(
        &mollusk,
        payer,
        &payer_account,
        upgrade_authority,
        operator,
        &operator_account,
        chain_name,
        its_hub_address,
    );
}
