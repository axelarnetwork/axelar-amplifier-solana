use anchor_lang::AccountDeserialize;
use anchor_lang::{Discriminator, InstructionData, Space, ToAccountMetas};
use anchor_spl::token::spl_token;
use anchor_spl::token_2022::spl_token_2022;
use mollusk_svm::program::keyed_account_for_system_program;
use mollusk_svm::result::Check;
use mollusk_svm::Mollusk;
use mollusk_test_utils::{
    create_program_data_account, get_event_authority_and_program_accounts, setup_mollusk,
};
use solana_axelar_gas_service::state::Treasury;
use solana_axelar_gateway::Message;
use solana_axelar_its::state::{InterchainTokenService, Roles, UserRoles};
use solana_axelar_operators::{OperatorAccount, OperatorRegistry};
use solana_sdk::native_token::LAMPORTS_PER_SOL;
use solana_sdk::program_pack::Pack;
use solana_sdk::{account::Account, instruction::Instruction, pubkey::Pubkey};

pub fn keyed_account_for_program(program_id: Pubkey) -> (Pubkey, Account) {
    (
        program_id,
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: vec![],
            owner: solana_sdk::bpf_loader_upgradeable::id(),
            executable: true,
            rent_epoch: 0,
        },
    )
}

pub fn new_test_account() -> (Pubkey, Account) {
    let pubkey = Pubkey::new_unique();
    let account = new_default_account();
    (pubkey, account)
}

pub fn new_default_account() -> Account {
    Account::new(LAMPORTS_PER_SOL, 0, &solana_sdk::system_program::ID)
}

pub fn new_empty_account() -> Account {
    Account::new(0, 0, &solana_sdk::system_program::ID)
}

pub fn get_message_signing_pda(message: &Message) -> (Pubkey, u8) {
    let program_id = solana_axelar_its::id();
    Pubkey::find_program_address(
        &[
            solana_axelar_gateway::seed_prefixes::VALIDATE_MESSAGE_SIGNING_SEED,
            message.command_id().as_ref(),
        ],
        &program_id,
    )
}

pub fn get_token_mint_pda(token_id: [u8; 32]) -> (Pubkey, u8) {
    let program_id = solana_axelar_its::id();
    let (its_root_pda, _) =
        Pubkey::find_program_address(&[InterchainTokenService::SEED_PREFIX], &program_id);

    Pubkey::find_program_address(
        &[
            solana_axelar_its::seed_prefixes::INTERCHAIN_TOKEN_SEED,
            its_root_pda.as_ref(),
            &token_id,
        ],
        &program_id,
    )
}

pub fn create_test_mint(mint_authority: Pubkey) -> (Pubkey, Account) {
    let mint = Pubkey::new_unique();
    let mint_data = {
        let mut data = [0u8; spl_token_2022::state::Mint::LEN];
        let mint_state = spl_token_2022::state::Mint {
            mint_authority: Some(mint_authority).into(),
            supply: 1_000_000_000, // 1 billion tokens
            decimals: 9,
            is_initialized: true,
            freeze_authority: Some(mint_authority).into(),
        };
        spl_token_2022::state::Mint::pack(mint_state, &mut data).unwrap();
        data.to_vec()
    };
    let rent = anchor_lang::prelude::Rent::default();
    let mint_account = Account {
        lamports: rent.minimum_balance(mint_data.len()),
        data: mint_data,
        owner: spl_token_2022::ID,
        executable: false,
        rent_epoch: 0,
    };

    (mint, mint_account)
}

pub fn create_rent_sysvar_data() -> Vec<u8> {
    use solana_sdk::rent::Rent;

    let rent = Rent::default();
    bincode::serialize(&rent).unwrap()
}

pub fn create_sysvar_instructions_data() -> Vec<u8> {
    use solana_sdk::sysvar::instructions::{construct_instructions_data, BorrowedInstruction};

    let instructions: &[BorrowedInstruction] = &[];
    construct_instructions_data(instructions)
}

pub fn initialize_mollusk_with_programs() -> Mollusk {
    let program_id = solana_axelar_its::id();

    // ITS
    let mut mollusk = setup_mollusk(&program_id, "solana_axelar_its");

    // Operators

    mollusk.add_program(
        &solana_axelar_operators::ID,
        "../../target/deploy/solana_axelar_operators",
        &solana_sdk::bpf_loader_upgradeable::ID,
    );

    // Gas Service

    mollusk.add_program(
        &solana_axelar_gas_service::ID,
        "../../target/deploy/solana_axelar_gas_service",
        &solana_sdk::bpf_loader_upgradeable::ID,
    );

    // Gateway

    mollusk.add_program(
        &solana_axelar_gateway::ID,
        "../../target/deploy/solana_axelar_gateway",
        &solana_sdk::bpf_loader_upgradeable::ID,
    );

    // Token Programs

    mollusk.add_program_with_elf_and_loader(
        &spl_token::ID,
        mollusk_svm_programs_token::token::ELF,
        &solana_sdk::bpf_loader_upgradeable::ID,
    );
    mollusk.add_program_with_elf_and_loader(
        &spl_token_2022::ID,
        mollusk_svm_programs_token::token2022::ELF,
        &solana_sdk::bpf_loader_upgradeable::ID,
    );
    mollusk.add_program_with_elf_and_loader(
        &anchor_spl::associated_token::ID,
        mollusk_svm_programs_token::associated_token::ELF,
        &solana_sdk::bpf_loader_upgradeable::ID,
    );
    mollusk.add_program(
        &mpl_token_metadata::programs::MPL_TOKEN_METADATA_ID,
        "../../programs/solana-axelar-its/tests/mpl_token_metadata",
        &solana_sdk::bpf_loader_upgradeable::id(),
    );

    mollusk
}

pub fn setup_operator(
    mollusk: &mut Mollusk,
    operator: Pubkey,
    operator_account: &Account,
) -> (Pubkey, Account) {
    let program_id = solana_axelar_operators::id();

    // Load the operators program into mollusk
    mollusk.add_program(
        &program_id,
        "solana_axelar_operators",
        &solana_sdk::bpf_loader_upgradeable::ID,
    );

    // Derive the registry PDA
    let (registry, _bump) = solana_axelar_operators::OperatorRegistry::find_pda();
    // Derive the operator PDA
    let (operator_pda, _bump) = solana_axelar_operators::OperatorAccount::find_pda(&operator);

    // Initialize the registry instruction
    let ix1 = Instruction {
        program_id,
        accounts: solana_axelar_operators::accounts::Initialize {
            payer: operator,
            owner: operator,
            registry,
            system_program: solana_sdk::system_program::ID,
        }
        .to_account_metas(None),
        data: solana_axelar_operators::instruction::Initialize {}.data(),
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
        accounts: solana_axelar_operators::accounts::AddOperator {
            owner: operator,
            operator_to_add: operator,
            registry,
            operator_account: operator_pda,
            system_program: solana_sdk::system_program::ID,
        }
        .to_account_metas(None),
        data: solana_axelar_operators::instruction::AddOperator {}.data(),
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
        (registry, new_empty_account()),
        (operator_pda, new_empty_account()),
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

pub fn init_gas_service(
    mollusk: &Mollusk,
    operator: Pubkey,
    operator_account: &Account,
    operator_pda: Pubkey,
    operator_pda_account: &Account,
) -> (Pubkey, Account) {
    let program_id = solana_axelar_gas_service::id();

    // Derive the treasury PDA
    let (treasury, _bump) = Pubkey::find_program_address(&[Treasury::SEED_PREFIX], &program_id);

    let ix = Instruction {
        program_id,
        accounts: solana_axelar_gas_service::accounts::Initialize {
            payer: operator,
            operator,
            operator_pda,
            treasury,
            system_program: solana_sdk::system_program::ID,
        }
        .to_account_metas(None),
        data: solana_axelar_gas_service::instruction::Initialize {}.data(),
    };

    let accounts = vec![
        (operator, operator_account.clone()),
        (operator_pda, operator_pda_account.clone()),
        (treasury, new_empty_account()),
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

#[allow(clippy::print_stdout)]
pub fn init_its_service(
    mollusk: &Mollusk,
    payer: Pubkey,
    payer_account: &Account,
    upgrade_authority: Pubkey,
    operator: Pubkey,
    operator_account: &Account,
    chain_name: String,
    its_hub_address: String,
) -> (Pubkey, Account, Pubkey, Account, Pubkey, Account) {
    let program_id = solana_axelar_its::id();

    // Derive the program data PDA for the upgradeable program
    let (program_data, _bump) = Pubkey::find_program_address(
        &[program_id.as_ref()],
        &solana_sdk::bpf_loader_upgradeable::ID,
    );
    let its_elf = mollusk_svm::file::load_program_elf("solana_axelar_its");
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

    let its_hub_addr_len = its_hub_address.len();
    let chain_name_len = chain_name.len();

    let ix = Instruction {
        program_id,
        accounts: solana_axelar_its::accounts::Initialize {
            payer,
            program_data,
            its_root_pda,
            system_program: solana_sdk::system_program::ID,
            operator,
            user_roles_account: user_roles_pda,
        }
        .to_account_metas(None),
        data: solana_axelar_its::instruction::Initialize {
            chain_name,
            its_hub_address,
        }
        .data(),
    };

    let accounts = vec![
        (payer, payer_account.clone()),
        (program_data, program_data_account.clone()),
        (its_root_pda, new_empty_account()),
        keyed_account_for_system_program(),
        (operator, operator_account.clone()),
        (user_roles_pda, new_empty_account()),
    ];

    let checks = vec![
        Check::success(),
        Check::account(&its_root_pda)
            .space(InterchainTokenService::space_for(
                its_hub_addr_len,
                chain_name_len,
                0,
            ))
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
    assert_eq!(
        user_roles_data.roles,
        Roles::OPERATOR,
        "user should be an operator"
    );

    (
        its_root_pda,
        its_root_account.clone(),
        user_roles_pda,
        user_roles_account.clone(),
        program_data,
        program_data_account,
    )
}

pub fn init_its_service_with_ethereum_trusted(
    mollusk: &Mollusk,
    payer: Pubkey,
    payer_account: &Account,
    upgrade_authority: Pubkey,
    operator: Pubkey,
    operator_account: &Account,
    chain_name: String,
    its_hub_address: String,
) -> (Pubkey, Account) {
    let program_id = solana_axelar_its::id();

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
    let trusted_chain_name = "ethereum".to_owned();

    let ix = Instruction {
        program_id,
        accounts: solana_axelar_its::accounts::SetTrustedChain {
            payer,
            user_roles: None,
            program_data: Some(program_data),
            its_root_pda,
            system_program: solana_sdk::system_program::ID,
            event_authority,
            program: program_id,
        }
        .to_account_metas(None),
        data: solana_axelar_its::instruction::SetTrustedChain {
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

    assert!(
        updated_its_data.is_trusted_chain("ethereum"),
        "Ethereum should be a trusted chain"
    );

    (its_root_pda, updated_its_account.clone())
}
