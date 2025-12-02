#![cfg(test)]
#![allow(clippy::too_many_lines)]

use anchor_lang::AccountDeserialize;
use mollusk_svm::result::Check;
use mollusk_test_utils::setup_mollusk;
use solana_axelar_gateway::seed_prefixes::GATEWAY_SEED;
use solana_axelar_gateway::ID as GATEWAY_PROGRAM_ID;
use solana_axelar_gateway_test_fixtures::initialize_gateway;
use solana_axelar_gateway_test_fixtures::setup_test_with_real_signers;
use solana_axelar_its::state::{token_manager::Type, TokenManager};
use solana_axelar_its::ItsError;
use solana_axelar_its_test_fixtures::new_test_account;
use solana_axelar_its_test_fixtures::{
    create_test_mint, execute_register_custom_token_helper, LinkTokenParams,
};
use solana_axelar_its_test_fixtures::{
    execute_link_token_helper, init_its_service_with_ethereum_trusted,
};
use solana_axelar_its_test_fixtures::{init_gas_service, LinkTokenContext};
use solana_axelar_its_test_fixtures::{
    initialize_mollusk_with_programs, RegisterCustomTokenContext,
};
use solana_axelar_its_test_fixtures::{setup_operator, RegisterCustomTokenParams};
use solana_sdk::pubkey::Pubkey;

#[test]
fn link_token() {
    let (setup, _, _, _, _) = setup_test_with_real_signers();
    let init_result = initialize_gateway(&setup);
    assert!(init_result.program_result.is_ok());

    let gas_service_program_id = solana_axelar_gas_service::id();
    let mut gas_service_mollusk =
        setup_mollusk(&gas_service_program_id, "solana_axelar_gas_service");

    let (gas_operator, gas_operator_account) = new_test_account();

    let (gas_operator_pda, gas_operator_pda_account) = setup_operator(
        &mut gas_service_mollusk,
        gas_operator,
        &gas_operator_account,
    );

    // Use the GAS SERVICE mollusk for gas service initialization
    let (_, treasury_account) = init_gas_service(
        &gas_service_mollusk,
        gas_operator,
        &gas_operator_account,
        gas_operator_pda,
        &gas_operator_pda_account,
    );

    let (gateway_root_pda, _) = Pubkey::find_program_address(&[GATEWAY_SEED], &GATEWAY_PROGRAM_ID);
    let gateway_root_pda_account = init_result.get_account(&gateway_root_pda).unwrap();

    let mollusk = initialize_mollusk_with_programs();

    let (payer, payer_account) = new_test_account();
    let (deployer, deployer_account) = new_test_account();
    let (its_operator, its_operator_account) = new_test_account();

    let chain_name = "solana".to_owned();
    let its_hub_address = "0x123456789abcdef".to_owned();

    let (its_root_pda, its_root_account) = init_its_service_with_ethereum_trusted(
        &mollusk,
        payer,
        &payer_account,
        payer,
        its_operator,
        &its_operator_account,
        chain_name.clone(),
        its_hub_address.clone(),
    );

    // Create a test mint (existing token to register)
    let mint_authority = Pubkey::new_unique();
    let (token_mint, token_mint_account) = create_test_mint(mint_authority);

    let register_ctx = RegisterCustomTokenContext {
        mollusk,
        payer: (payer, payer_account.clone()),
        deployer: (deployer, deployer_account.clone()),
        its_root: (its_root_pda, its_root_account.clone()),
        token_mint: (token_mint, token_mint_account),
    };

    let salt = [2u8; 32];
    let token_manager_type = Type::LockUnlock;

    let register_params = RegisterCustomTokenParams {
        salt,
        token_manager_type,
        operator: None, // No operator for this test
    };

    let register_checks = vec![Check::success()];

    let register_result =
        execute_register_custom_token_helper(register_ctx, register_params, register_checks);

    assert!(register_result.result.program_result.is_ok());

    let token_manager_pda = register_result.token_manager_pda;
    let mollusk = register_result.mollusk;

    // Get the updated token manager account after registration
    let token_manager_account = register_result
        .result
        .get_account(&token_manager_pda)
        .unwrap();

    // Verify token manager was created correctly
    let token_manager =
        TokenManager::try_deserialize(&mut token_manager_account.data.as_ref()).unwrap();
    assert_eq!(token_manager.ty, Type::LockUnlock);

    // Derive required PDAs
    let (gas_treasury, _) = Pubkey::find_program_address(
        &[solana_axelar_gas_service::state::Treasury::SEED_PREFIX],
        &solana_axelar_gas_service::ID,
    );

    // Now use the helper function for the link token part
    let ctx = LinkTokenContext {
        mollusk,
        payer: (payer, payer_account),
        deployer: (deployer, deployer_account),
        its_root: (its_root_pda, its_root_account),
        token_manager: (token_manager_pda, token_manager_account.clone()),
        gateway_root: (gateway_root_pda, gateway_root_pda_account.clone()),
        gas_treasury: (gas_treasury, treasury_account),
    };

    let params = LinkTokenParams {
        salt,
        destination_chain: "ethereum".to_owned(),
        destination_token_address: vec![0x12, 0x34, 0x56, 0x78],
        token_manager_type,
        link_params: vec![],
        gas_value: 0u64,
    };

    let link_result = execute_link_token_helper(ctx, params, vec![Check::success()]);

    assert!(link_result.result.program_result.is_ok());
}

#[test]
fn reject_link_token_untrusted_destination_chain() {
    let (setup, _, _, _, _) = setup_test_with_real_signers();
    let init_result = initialize_gateway(&setup);
    assert!(init_result.program_result.is_ok());

    let gas_service_program_id = solana_axelar_gas_service::id();
    let mut gas_service_mollusk =
        setup_mollusk(&gas_service_program_id, "solana_axelar_gas_service");

    let (gas_operator, gas_operator_account) = new_test_account();

    let (gas_operator_pda, gas_operator_pda_account) = setup_operator(
        &mut gas_service_mollusk,
        gas_operator,
        &gas_operator_account,
    );

    // Use the GAS SERVICE mollusk for gas service initialization
    let (_, treasury_account) = init_gas_service(
        &gas_service_mollusk,
        gas_operator,
        &gas_operator_account,
        gas_operator_pda,
        &gas_operator_pda_account,
    );

    let (gateway_root_pda, _) = Pubkey::find_program_address(&[GATEWAY_SEED], &GATEWAY_PROGRAM_ID);
    let gateway_root_pda_account = init_result.get_account(&gateway_root_pda).unwrap();

    let mollusk = initialize_mollusk_with_programs();

    let (payer, payer_account) = new_test_account();
    let (deployer, deployer_account) = new_test_account();
    let (its_operator, its_operator_account) = new_test_account();

    let chain_name = "solana".to_owned();
    let its_hub_address = "0x123456789abcdef".to_owned();

    let (its_root_pda, its_root_account) = init_its_service_with_ethereum_trusted(
        &mollusk,
        payer,
        &payer_account,
        payer,
        its_operator,
        &its_operator_account,
        chain_name.clone(),
        its_hub_address.clone(),
    );

    // Create a test mint (existing token to register)
    let mint_authority = Pubkey::new_unique();
    let (token_mint, token_mint_account) = create_test_mint(mint_authority);

    let salt = [2u8; 32];
    let token_manager_type = Type::LockUnlock; // Use LockUnlock, NOT NativeInterchainToken

    let register_ctx = RegisterCustomTokenContext {
        mollusk,
        payer: (payer, payer_account.clone()),
        deployer: (deployer, deployer_account.clone()),
        its_root: (its_root_pda, its_root_account.clone()),
        token_mint: (token_mint, token_mint_account),
    };

    let register_params = RegisterCustomTokenParams {
        salt,
        token_manager_type,
        operator: None, // No operator
    };

    let register_result =
        execute_register_custom_token_helper(register_ctx, register_params, vec![Check::success()]);

    assert!(register_result.result.program_result.is_ok());

    let token_manager_pda = register_result.token_manager_pda;
    let mollusk = register_result.mollusk;

    let token_manager_account = register_result
        .result
        .get_account(&token_manager_pda)
        .unwrap();

    let token_manager =
        TokenManager::try_deserialize(&mut token_manager_account.data.as_ref()).unwrap();
    assert_eq!(token_manager.ty, Type::LockUnlock);

    let (gas_treasury, _) = Pubkey::find_program_address(
        &[solana_axelar_gas_service::state::Treasury::SEED_PREFIX],
        &solana_axelar_gas_service::ID,
    );

    let ctx = LinkTokenContext {
        mollusk,
        payer: (payer, payer_account),
        deployer: (deployer, deployer_account),
        its_root: (its_root_pda, its_root_account),
        token_manager: (token_manager_pda, token_manager_account.clone()),
        gateway_root: (gateway_root_pda, gateway_root_pda_account.clone()),
        gas_treasury: (gas_treasury, treasury_account),
    };

    let params = LinkTokenParams {
        salt,
        destination_chain: "untrusted chain".to_owned(),
        destination_token_address: vec![0x12, 0x34, 0x56, 0x78],
        token_manager_type,
        link_params: vec![],
        gas_value: 0u64,
    };

    let checks = vec![Check::err(
        anchor_lang::error::Error::from(ItsError::UntrustedDestinationChain).into(),
    )];

    let link_result = execute_link_token_helper(ctx, params, checks);

    assert!(link_result.result.program_result.is_err());
}

#[test]
fn reject_link_token_invalid_destination_chain() {
    let (setup, _, _, _, _) = setup_test_with_real_signers();
    let init_result = initialize_gateway(&setup);
    assert!(init_result.program_result.is_ok());

    let gas_service_program_id = solana_axelar_gas_service::id();
    let mut gas_service_mollusk =
        setup_mollusk(&gas_service_program_id, "solana_axelar_gas_service");

    let (gas_operator, gas_operator_account) = new_test_account();

    let (gas_operator_pda, gas_operator_pda_account) = setup_operator(
        &mut gas_service_mollusk,
        gas_operator,
        &gas_operator_account,
    );

    let (_, treasury_account) = init_gas_service(
        &gas_service_mollusk,
        gas_operator,
        &gas_operator_account,
        gas_operator_pda,
        &gas_operator_pda_account,
    );

    let (gateway_root_pda, _) = Pubkey::find_program_address(&[GATEWAY_SEED], &GATEWAY_PROGRAM_ID);
    let gateway_root_pda_account = init_result.get_account(&gateway_root_pda).unwrap();

    let mollusk = initialize_mollusk_with_programs();

    let (payer, payer_account) = new_test_account();
    let (deployer, deployer_account) = new_test_account();
    let (its_operator, its_operator_account) = new_test_account();

    let chain_name = "solana".to_owned();
    let its_hub_address = "0x123456789abcdef".to_owned();

    let (its_root_pda, its_root_account) = init_its_service_with_ethereum_trusted(
        &mollusk,
        payer,
        &payer_account,
        payer,
        its_operator,
        &its_operator_account,
        chain_name.clone(),
        its_hub_address.clone(),
    );

    // Create a test mint (existing token to register)
    let mint_authority = Pubkey::new_unique();
    let (token_mint, token_mint_account) = create_test_mint(mint_authority);

    // Register custom token parameters
    let salt = [2u8; 32];
    let token_manager_type = Type::LockUnlock;

    let register_ctx = RegisterCustomTokenContext {
        mollusk,
        payer: (payer, payer_account.clone()),
        deployer: (deployer, deployer_account.clone()),
        its_root: (its_root_pda, its_root_account.clone()),
        token_mint: (token_mint, token_mint_account),
    };

    let register_params = RegisterCustomTokenParams {
        salt,
        token_manager_type,
        operator: None,
    };

    let register_result =
        execute_register_custom_token_helper(register_ctx, register_params, vec![Check::success()]);

    assert!(register_result.result.program_result.is_ok());

    let token_manager_pda = register_result.token_manager_pda;
    let mollusk = register_result.mollusk;

    // Get the updated token manager account after registration
    let token_manager_account = register_result
        .result
        .get_account(&token_manager_pda)
        .unwrap();

    // Verify token manager was created correctly
    let token_manager =
        TokenManager::try_deserialize(&mut token_manager_account.data.as_ref()).unwrap();
    assert_eq!(token_manager.ty, Type::LockUnlock);

    // Derive required PDAs
    let (gas_treasury, _) = Pubkey::find_program_address(
        &[solana_axelar_gas_service::state::Treasury::SEED_PREFIX],
        &solana_axelar_gas_service::ID,
    );

    let ctx = LinkTokenContext {
        mollusk,
        payer: (payer, payer_account),
        deployer: (deployer, deployer_account),
        its_root: (its_root_pda, its_root_account),
        token_manager: (token_manager_pda, token_manager_account.clone()),
        gateway_root: (gateway_root_pda, gateway_root_pda_account.clone()),
        gas_treasury: (gas_treasury, treasury_account),
    };

    let params = LinkTokenParams {
        salt,
        destination_chain: "solana".to_owned(), // INVALID: same destination chain as source chain
        destination_token_address: vec![0x12, 0x34, 0x56, 0x78],
        token_manager_type,
        link_params: vec![],
        gas_value: 0u64,
    };

    let checks = vec![Check::err(
        anchor_lang::error::Error::from(ItsError::InvalidDestinationChain).into(),
    )];

    let link_result = execute_link_token_helper(ctx, params, checks);

    assert!(link_result.result.program_result.is_err());
}
