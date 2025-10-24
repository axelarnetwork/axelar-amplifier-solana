use crate::initialize::init_gas_service;
use crate::initialize::init_its_service_with_ethereum_trusted;
use crate::initialize::setup_operator;
use anchor_lang::system_program;
use anchor_lang::InstructionData;
use anchor_lang::ToAccountMetas;
use anchor_spl::token_2022::spl_token_2022;
use axelar_solana_gateway_v2::seed_prefixes::{CALL_CONTRACT_SIGNING_SEED, GATEWAY_SEED};
use axelar_solana_gateway_v2::ID as GATEWAY_PROGRAM_ID;
use axelar_solana_gateway_v2_test_fixtures::initialize_gateway;
use axelar_solana_gateway_v2_test_fixtures::setup_test_with_real_signers;
use mollusk_svm::result::Check;
use mollusk_test_utils::{get_event_authority_and_program_accounts, setup_mollusk};
use solana_program::program_pack::Pack;
use solana_sdk::{
    account::Account, instruction::Instruction, native_token::LAMPORTS_PER_SOL, pubkey::Pubkey,
};
use spl_token_2022::state::Mint;

#[path = "initialize.rs"]
mod initialize;

#[test]
fn test_register_token_metadata() {
    // Initialize gateway
    let (setup, _, _, _, _) = setup_test_with_real_signers();

    let init_result = initialize_gateway(&setup);
    assert!(init_result.program_result.is_ok());

    // Initialize gas service
    let gas_service_program_id = axelar_solana_gas_service_v2::id();
    let mut mollusk = setup_mollusk(&gas_service_program_id, "axelar_solana_gas_service_v2");

    let operator = Pubkey::new_unique();
    let operator_account = Account::new(1_000_000_000, 0, &system_program::ID);

    let (operator_pda, operator_pda_account) =
        setup_operator(&mut mollusk, operator, &operator_account);

    let (treasury_pda, treasury_account) = init_gas_service(
        &mollusk,
        operator,
        &operator_account,
        operator_pda,
        &operator_pda_account,
    );

    let (gateway_root_pda, _) = Pubkey::find_program_address(&[GATEWAY_SEED], &GATEWAY_PROGRAM_ID);
    let gateway_root_pda_account = init_result.get_account(&gateway_root_pda).unwrap();

    let program_id = axelar_solana_its_v2::id();
    let mollusk = initialize::initialize_mollusk();

    let payer = Pubkey::new_unique();
    let payer_account = Account::new(10 * LAMPORTS_PER_SOL, 0, &system_program::ID);

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

    // Create a simple SPL Token 2022 mint for testing
    let mint_authority = Pubkey::new_unique();
    let token_mint = Pubkey::new_unique();
    let decimals = 9u8;

    // Create dummy mint account data using SPL Token 2022 Pack trait
    let mint_data = {
        let mint = Mint {
            mint_authority: solana_program::program_option::COption::Some(mint_authority),
            supply: 0,
            decimals,
            is_initialized: true,
            freeze_authority: solana_program::program_option::COption::None,
        };
        let mut data = vec![0u8; Mint::LEN];
        Mint::pack(mint, &mut data).unwrap();
        data
    };

    let mint_account = Account {
        lamports: 1_000_000_000,
        data: mint_data,
        owner: spl_token_2022::id(),
        executable: false,
        rent_epoch: 0,
    };

    // Derive signing PDA for call contract
    let (call_contract_signing_pda, signing_pda_bump) =
        Pubkey::find_program_address(&[CALL_CONTRACT_SIGNING_SEED], &program_id);

    // Get event authority accounts
    let (gateway_event_authority, gateway_event_authority_account, gateway_program_account) =
        get_event_authority_and_program_accounts(&axelar_solana_gateway_v2::ID);

    let (gas_event_authority, gas_event_authority_account, gas_service_program_account) =
        get_event_authority_and_program_accounts(&axelar_solana_gas_service_v2::ID);

    let (event_authority, event_authority_account, program_account) =
        get_event_authority_and_program_accounts(&program_id);

    // Test with no gas payment
    let gas_value = 0u64;

    let ix = Instruction {
        program_id,
        accounts: axelar_solana_its_v2::accounts::RegisterTokenMetadata {
            payer,
            token_mint,
            gateway_root_pda,
            axelar_gateway_program: axelar_solana_gateway_v2::ID,
            gas_treasury: treasury_pda,
            gas_service: axelar_solana_gas_service_v2::ID,
            system_program: system_program::ID,
            its_root_pda,
            call_contract_signing_pda,
            its_program: program_id,
            gateway_event_authority,
            gas_event_authority,
            // event CPI
            event_authority,
            program: program_id,
        }
        .to_account_metas(None),
        data: axelar_solana_its_v2::instruction::RegisterTokenMetadata {
            gas_value,
            signing_pda_bump,
        }
        .data(),
    };

    let accounts = vec![
        (payer, payer_account.clone()),
        (token_mint, mint_account),
        (gateway_root_pda, gateway_root_pda_account.clone()),
        (axelar_solana_gateway_v2::ID, gateway_program_account),
        (treasury_pda, treasury_account),
        (
            axelar_solana_gas_service_v2::ID,
            gas_service_program_account,
        ),
        mollusk_svm::program::keyed_account_for_system_program(),
        (its_root_pda, its_root_account),
        (
            call_contract_signing_pda,
            Account::new(0, 0, &system_program::ID),
        ),
        (program_id, program_account),
        (gateway_event_authority, gateway_event_authority_account),
        (gas_event_authority, gas_event_authority_account),
        // for event cpi
        (event_authority, event_authority_account),
        (program_id, Account::new(0, 0, &system_program::ID)),
    ];

    let checks = vec![Check::success()];

    let result = mollusk.process_and_validate_instruction(&ix, &accounts, &checks);

    assert!(
        result.program_result.is_ok(),
        "Register token metadata instruction should succeed: {:?}",
        result.program_result
    );
}
