use anchor_lang::AccountDeserialize;
use anchor_lang::Discriminator;
use anchor_lang::InstructionData;
use anchor_lang::Space;
use anchor_lang::ToAccountMetas;
use anchor_spl::{
    token::spl_token,
    token_2022::spl_token_2022::{self},
};
use axelar_solana_gas_service_v2::state::Treasury;
use axelar_solana_gateway_v2::seed_prefixes::CALL_CONTRACT_SIGNING_SEED;
use axelar_solana_gateway_v2::seed_prefixes::GATEWAY_SEED;
use axelar_solana_gateway_v2::ID as GATEWAY_PROGRAM_ID;
use axelar_solana_gateway_v2_test_fixtures::initialize_gateway;
use axelar_solana_gateway_v2_test_fixtures::setup_test_with_real_signers;
use axelar_solana_its_v2::state::InterchainTokenService;
use axelar_solana_its_v2_test_fixtures::{
    deploy_interchain_token_helper, DeployInterchainTokenContext,
};
use axelar_solana_operators::OperatorAccount;
use axelar_solana_operators::OperatorRegistry;
use mollusk_svm::program::keyed_account_for_system_program;
use mollusk_svm::result::Check;
use mollusk_svm::Mollusk;
use mollusk_svm_programs_token;
use mollusk_test_utils::{get_event_authority_and_program_accounts, setup_mollusk};
use solana_sdk::{
    account::Account, native_token::LAMPORTS_PER_SOL, pubkey::Pubkey, system_program,
};

mod initialize;
use spl_token_metadata_interface::solana_instruction::Instruction;

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
    let program_id = axelar_solana_its_v2::id();

    // First initialize the ITS service
    let (
        its_root_pda,
        its_root_account,
        _user_roles_pda,
        _user_roles_account,
        program_data,
        program_data_account,
    ) = crate::initialize::init_its_service(
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
fn test_deploy_remote_interchain_token() {
    // Initialize gateway
    let (setup, _, _, _, _) = setup_test_with_real_signers();

    let init_result = initialize_gateway(&setup);
    assert!(!init_result.program_result.is_err());

    // Initialize gas service
    let gas_service_program_id = axelar_solana_gas_service_v2::id();
    let mut mollusk = setup_mollusk(&gas_service_program_id, "axelar_solana_gas_service_v2");

    let operator = Pubkey::new_unique();
    let operator_account = Account::new(1_000_000_000, 0, &system_program::ID);

    let (operator_pda, operator_pda_account) =
        setup_operator(&mut mollusk, operator, &operator_account);

    let (_, treasury_pda) = init_gas_service(
        &mollusk,
        operator,
        &operator_account,
        operator_pda,
        &operator_pda_account,
    );

    let (gateway_root_pda, _) = Pubkey::find_program_address(&[GATEWAY_SEED], &GATEWAY_PROGRAM_ID);
    let gateway_root_pda_account = init_result.get_account(&gateway_root_pda).unwrap();

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

    let payer = Pubkey::new_unique();
    let payer_account = Account::new(10 * LAMPORTS_PER_SOL, 0, &system_program::ID);

    let deployer = Pubkey::new_unique();
    let deployer_account = Account::new(10 * LAMPORTS_PER_SOL, 0, &system_program::ID);

    let operator = Pubkey::new_unique();
    let operator_account = Account::new(1_000_000_000, 0, &system_program::ID);

    let chain_name = "solana".to_string();
    let its_hub_address = "0x123456789abcdef".to_string();

    let (its_root_pda, its_root_account) = init_its_service_with_ethereum_trusted(
        &mollusk,
        payer,
        &payer_account,
        payer,
        operator,
        &operator_account,
        chain_name.clone(),
        its_hub_address.clone(),
    );

    // Create simple token deployment parameters
    let salt = [1u8; 32];
    let name = "Test Token".to_string();
    let symbol = "TEST".to_string();
    let decimals = 9u8;
    let initial_supply = 1_000_000_000u64; // 1 billion tokens with 9 decimals
    let minter = None;

    let ctx = DeployInterchainTokenContext::new(
        mollusk,
        its_root_pda,
        its_root_account,
        deployer,
        deployer_account,
        program_id,
        payer,
        payer_account,
    );

    let (
        result,
        token_manager_pda,
        token_mint_pda,
        _token_manager_ata,
        _deployer_ata,
        metadata_account,
        mollusk,
    ) = deploy_interchain_token_helper(
        salt,
        name.clone(),
        symbol.clone(),
        decimals,
        initial_supply,
        minter,
        ctx,
    );

    assert!(
        result.program_result.is_ok(),
        "Deploy interchain token instruction should succeed: {:?}",
        result.program_result
    );

    let destination_chain = "ethereum".to_string();
    let gas_value = 0u64;

    let (gateway_root_pda, _) = Pubkey::find_program_address(
        &[axelar_solana_gateway_v2::seed_prefixes::GATEWAY_SEED],
        &axelar_solana_gateway_v2::ID,
    );

    let (gas_treasury, _) =
        Pubkey::find_program_address(&[Treasury::SEED_PREFIX], &axelar_solana_gas_service_v2::ID);

    let (call_contract_signing_pda, signing_pda_bump) =
        Pubkey::find_program_address(&[CALL_CONTRACT_SIGNING_SEED], &program_id);

    let (gateway_event_authority, _) =
        Pubkey::find_program_address(&[b"__event_authority"], &axelar_solana_gateway_v2::ID);

    let (gas_event_authority, _) =
        Pubkey::find_program_address(&[b"__event_authority"], &axelar_solana_gas_service_v2::ID);

    let (its_event_authority, its_event_authority_account, its_program_account) =
        get_event_authority_and_program_accounts(&program_id);

    let ix = Instruction {
        program_id,
        accounts: axelar_solana_its_v2::accounts::DeployRemoteInterchainToken {
            payer,
            deployer,
            token_mint: token_mint_pda,
            metadata_account,
            token_manager_pda,
            gateway_root_pda,
            axelar_gateway_program: axelar_solana_gateway_v2::ID,
            gas_treasury,
            gas_service: axelar_solana_gas_service_v2::ID,
            system_program: system_program::ID,
            its_root_pda,
            call_contract_signing_pda,
            its_program: program_id,
            gateway_event_authority,
            gas_event_authority,
            event_authority: its_event_authority,
            program: program_id,
        }
        .to_account_metas(None),
        data: axelar_solana_its_v2::instruction::DeployRemoteInterchainToken {
            salt,
            destination_chain: destination_chain.clone(),
            gas_value,
            signing_pda_bump,
        }
        .data(),
    };

    // Get the updated accounts from the first instruction result
    let updated_mollusk = mollusk;
    let updated_payer_account = result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == payer)
        .map(|(_, account)| account.clone())
        .unwrap_or_else(|| Account::new(9 * LAMPORTS_PER_SOL, 0, &system_program::ID));

    let updated_its_root_account = result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == its_root_pda)
        .unwrap()
        .1
        .clone();

    let updated_token_mint_account = result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == token_mint_pda)
        .unwrap()
        .1
        .clone();

    let updated_metadata_account = result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == metadata_account)
        .unwrap()
        .1
        .clone();

    let updated_token_manager_account = result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == token_manager_pda)
        .unwrap()
        .1
        .clone();

    // Accounts for the deploy remote instruction
    let accounts = vec![
        (payer, updated_payer_account),
        (
            deployer,
            Account::new(10 * LAMPORTS_PER_SOL, 0, &system_program::ID),
        ),
        (token_mint_pda, updated_token_mint_account),
        (metadata_account, updated_metadata_account),
        (token_manager_pda, updated_token_manager_account),
        (gateway_root_pda, gateway_root_pda_account.clone()),
        (
            axelar_solana_gateway_v2::ID,
            Account {
                lamports: 1,
                data: vec![],
                owner: solana_sdk::bpf_loader_upgradeable::ID,
                executable: true,
                rent_epoch: 0,
            },
        ),
        (gas_treasury, treasury_pda),
        (
            axelar_solana_gas_service_v2::ID,
            Account {
                lamports: 1,
                data: vec![],
                owner: solana_sdk::bpf_loader_upgradeable::ID,
                executable: true,
                rent_epoch: 0,
            },
        ),
        (
            system_program::ID,
            Account {
                lamports: 1,
                data: vec![],
                owner: solana_sdk::native_loader::id(),
                executable: true,
                rent_epoch: 0,
            },
        ),
        (its_root_pda, updated_its_root_account),
        (call_contract_signing_pda, Account::new(0, 0, &program_id)),
        (
            program_id,
            Account {
                lamports: 1,
                data: vec![],
                owner: solana_sdk::bpf_loader_upgradeable::ID,
                executable: true,
                rent_epoch: 0,
            },
        ),
        (
            gateway_event_authority,
            Account::new(0, 0, &system_program::ID),
        ),
        (gas_event_authority, Account::new(0, 0, &system_program::ID)),
        // For event cpi
        (its_event_authority, its_event_authority_account),
        (program_id, its_program_account),
    ];

    let remote_result = updated_mollusk.process_instruction(&ix, &accounts);

    assert!(
        remote_result.program_result.is_ok(),
        "Deploy remote interchain token instruction should succeed: {:?}",
        remote_result.program_result
    );
}
