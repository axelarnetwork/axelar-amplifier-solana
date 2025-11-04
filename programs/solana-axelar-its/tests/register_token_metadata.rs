#![cfg(test)]
#![allow(clippy::too_many_lines)]

use anchor_lang::solana_program;
use anchor_lang::system_program;
use anchor_lang::InstructionData;
use anchor_lang::ToAccountMetas;
use anchor_spl::token_2022::spl_token_2022;
use mollusk_svm::result::Check;
use mollusk_test_utils::{get_event_authority_and_program_accounts, setup_mollusk};
use solana_axelar_gateway::seed_prefixes::{CALL_CONTRACT_SIGNING_SEED, GATEWAY_SEED};
use solana_axelar_gateway::ID as GATEWAY_PROGRAM_ID;
use solana_axelar_gateway_test_fixtures::initialize_gateway;
use solana_axelar_gateway_test_fixtures::setup_test_with_real_signers;
use solana_axelar_its_test_fixtures::init_gas_service;
use solana_axelar_its_test_fixtures::init_its_service_with_ethereum_trusted;
use solana_axelar_its_test_fixtures::initialize_mollusk;
use solana_axelar_its_test_fixtures::setup_operator;
use solana_program::program_pack::Pack;
use solana_sdk::{
    account::Account, instruction::Instruction, native_token::LAMPORTS_PER_SOL, pubkey::Pubkey,
};
use spl_token_2022::state::Mint;

#[test]
fn test_register_token_metadata() {
    // Initialize gateway
    let (setup, _, _, _, _) = setup_test_with_real_signers();

    let init_result = initialize_gateway(&setup);
    assert!(init_result.program_result.is_ok());

    // Initialize gas service
    let gas_service_program_id = solana_axelar_gas_service::id();
    let mut mollusk = setup_mollusk(&gas_service_program_id, "solana_axelar_gas_service");

    let operator = Pubkey::new_unique();
    let operator_account = Account::new(1_000_000_000, 0, &solana_sdk::system_program::ID);

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

    let program_id = solana_axelar_its::id();
    let mollusk = initialize_mollusk();

    let payer = Pubkey::new_unique();
    let payer_account = Account::new(10 * LAMPORTS_PER_SOL, 0, &solana_sdk::system_program::ID);

    let operator = Pubkey::new_unique();
    let operator_account = Account::new(1_000_000_000, 0, &solana_sdk::system_program::ID);

    let chain_name = "solana".to_owned();
    let its_hub_address = "0x123456789abcdef".to_owned();

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
    let (call_contract_signing_pda, _signing_pda_bump) =
        Pubkey::find_program_address(&[CALL_CONTRACT_SIGNING_SEED], &program_id);

    // Get event authority accounts
    let (gateway_event_authority, gateway_event_authority_account, gateway_program_account) =
        get_event_authority_and_program_accounts(&solana_axelar_gateway::ID);

    let (gas_event_authority, gas_event_authority_account, gas_service_program_account) =
        get_event_authority_and_program_accounts(&solana_axelar_gas_service::ID);

    let (event_authority, event_authority_account, program_account) =
        get_event_authority_and_program_accounts(&program_id);

    // Test with no gas payment
    let gas_value = 0u64;

    let ix = Instruction {
        program_id,
        accounts: solana_axelar_its::accounts::RegisterTokenMetadata {
            payer,
            token_mint,
            gateway_root_pda,
            gateway_program: solana_axelar_gateway::ID,
            system_program: system_program::ID,
            its_root_pda,
            call_contract_signing_pda,
            gateway_event_authority,
            gas_service_accounts: solana_axelar_its::accounts::GasServiceAccounts {
                gas_treasury: treasury_pda,
                gas_service: solana_axelar_gas_service::ID,
                gas_event_authority,
            },
            event_authority,
            program: program_id,
        }
        .to_account_metas(None),
        data: solana_axelar_its::instruction::RegisterTokenMetadata { gas_value }.data(),
    };

    let accounts = vec![
        (payer, payer_account.clone()),
        (token_mint, mint_account),
        (gateway_root_pda, gateway_root_pda_account.clone()),
        (solana_axelar_gateway::ID, gateway_program_account),
        (treasury_pda, treasury_account),
        (solana_axelar_gas_service::ID, gas_service_program_account),
        mollusk_svm::program::keyed_account_for_system_program(),
        (its_root_pda, its_root_account),
        (
            call_contract_signing_pda,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
        (program_id, program_account),
        (gateway_event_authority, gateway_event_authority_account),
        (gas_event_authority, gas_event_authority_account),
        // for event cpi
        (event_authority, event_authority_account),
        (
            program_id,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
    ];

    let checks = vec![Check::success()];

    let result = mollusk.process_and_validate_instruction(&ix, &accounts, &checks);

    assert!(
        result.program_result.is_ok(),
        "Register token metadata instruction should succeed: {:?}",
        result.program_result
    );
}
