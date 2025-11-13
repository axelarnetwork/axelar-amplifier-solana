#![cfg(test)]
#![allow(clippy::too_many_lines)]

use mollusk_svm::result::Check;
use mollusk_test_utils::setup_mollusk;
use solana_axelar_gateway::seed_prefixes::GATEWAY_SEED;
use solana_axelar_gateway::ID as GATEWAY_PROGRAM_ID;
use solana_axelar_gateway_test_fixtures::initialize_gateway;
use solana_axelar_gateway_test_fixtures::setup_test_with_real_signers;
use solana_axelar_its::seed_prefixes::INTERCHAIN_TOKEN_SEED;
use solana_axelar_its::ItsError;
use solana_axelar_its_test_fixtures::deploy_remote_interchain_token_helper;
use solana_axelar_its_test_fixtures::init_gas_service;
use solana_axelar_its_test_fixtures::init_its_service_with_ethereum_trusted;
use solana_axelar_its_test_fixtures::initialize_mollusk_with_programs;
use solana_axelar_its_test_fixtures::new_test_account;
use solana_axelar_its_test_fixtures::setup_operator;
use solana_axelar_its_test_fixtures::DeployRemoteInterchainTokenContext;
use solana_axelar_its_test_fixtures::{
    deploy_interchain_token_helper, DeployInterchainTokenContext,
};
use solana_sdk::{account::Account, pubkey::Pubkey};

#[test]
fn test_deploy_remote_interchain_token() {
    let (setup, _, _, _, _) = setup_test_with_real_signers();

    let init_result = initialize_gateway(&setup);
    assert!(init_result.program_result.is_ok());

    let gas_service_program_id = solana_axelar_gas_service::id();
    let mut mollusk = setup_mollusk(&gas_service_program_id, "solana_axelar_gas_service");

    let (operator, operator_account) = new_test_account();

    let (operator_pda, operator_pda_account) =
        setup_operator(&mut mollusk, operator, &operator_account);

    let (_, treasury_pda_account) = init_gas_service(
        &mollusk,
        operator,
        &operator_account,
        operator_pda,
        &operator_pda_account,
    );

    let (gateway_root_pda, _) = Pubkey::find_program_address(&[GATEWAY_SEED], &GATEWAY_PROGRAM_ID);
    let gateway_root_pda_account = init_result.get_account(&gateway_root_pda).unwrap();

    let program_id = solana_axelar_its::id();
    let mollusk = initialize_mollusk_with_programs();

    let (payer, payer_account) = new_test_account();
    let (deployer, deployer_account) = new_test_account();
    let (operator, operator_account) = new_test_account();

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

    // Create simple token deployment parameters
    let salt = [1u8; 32];
    let name = "Test Token".to_owned();
    let symbol = "TEST".to_owned();
    let decimals = 9u8;
    let initial_supply = 1_000_000_000u64; // 1 billion tokens with 9 decimals

    let ctx = DeployInterchainTokenContext::new(
        mollusk,
        (its_root_pda, its_root_account),
        (deployer, deployer_account),
        (payer, payer_account.clone()),
        None,
        None,
    );

    let (result, token_manager_pda, token_mint_pda, _, _, metadata_account, mollusk) =
        deploy_interchain_token_helper(
            ctx,
            salt,
            name.clone(),
            symbol.clone(),
            decimals,
            initial_supply,
            vec![Check::success()],
        );

    assert!(result.program_result.is_ok());

    let destination_chain = "ethereum".to_owned();
    let gas_value = 0u64;

    let token_mint_account = result.get_account(&token_mint_pda).unwrap().clone();
    let metadata_account_data = result.get_account(&metadata_account).unwrap().clone();
    let token_manager_account = result.get_account(&token_manager_pda).unwrap().clone();
    let its_root_account = result.get_account(&its_root_pda).unwrap().clone();

    let ctx = DeployRemoteInterchainTokenContext::new(
        mollusk,
        program_id,
        (payer, payer_account),
        deployer,
        (token_mint_pda, token_mint_account),
        (metadata_account, metadata_account_data),
        (token_manager_pda, token_manager_account),
        (its_root_pda, its_root_account),
        treasury_pda_account,
        gateway_root_pda_account.clone(),
    );

    let remote_result = deploy_remote_interchain_token_helper(
        salt,
        destination_chain.clone(),
        gas_value,
        ctx,
        vec![Check::success()],
    );

    assert!(remote_result.program_result.is_ok());
}

#[test]
fn test_reject_deploy_remote_interchain_token_with_no_token_manager() {
    let (setup, _, _, _, _) = setup_test_with_real_signers();

    let init_result = initialize_gateway(&setup);
    assert!(init_result.program_result.is_ok());

    let gas_service_program_id = solana_axelar_gas_service::id();
    let mut mollusk = setup_mollusk(&gas_service_program_id, "solana_axelar_gas_service");

    let (operator, operator_account) = new_test_account();

    let (operator_pda, operator_pda_account) =
        setup_operator(&mut mollusk, operator, &operator_account);

    let (_, treasury_pda_account) = init_gas_service(
        &mollusk,
        operator,
        &operator_account,
        operator_pda,
        &operator_pda_account,
    );

    let (gateway_root_pda, _) = Pubkey::find_program_address(&[GATEWAY_SEED], &GATEWAY_PROGRAM_ID);
    let gateway_root_pda_account = init_result.get_account(&gateway_root_pda).unwrap();

    let program_id = solana_axelar_its::id();
    let mollusk = initialize_mollusk_with_programs();

    let (payer, payer_account) = new_test_account();
    let (deployer, deployer_account) = new_test_account();
    let (operator, operator_account) = new_test_account();

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

    // Create simple token deployment parameters
    let salt = [1u8; 32];
    let name = "Test Token".to_owned();
    let symbol = "TEST".to_owned();
    let decimals = 9u8;
    let initial_supply = 1_000_000_000u64; // 1 billion tokens with 9 decimals

    let ctx = DeployInterchainTokenContext::new(
        mollusk,
        (its_root_pda, its_root_account),
        (deployer, deployer_account),
        (payer, payer_account.clone()),
        None,
        None,
    );

    let (result, token_manager_pda, token_mint_pda, _, _, metadata_account, mollusk) =
        deploy_interchain_token_helper(
            ctx,
            salt,
            name.clone(),
            symbol.clone(),
            decimals,
            initial_supply,
            vec![Check::success()],
        );

    assert!(result.program_result.is_ok());

    let destination_chain = "ethereum".to_owned();
    let gas_value = 0u64;

    let token_mint_account = result.get_account(&token_mint_pda).unwrap().clone();
    let metadata_account_data = result.get_account(&metadata_account).unwrap().clone();
    let modified_token_manager_account = Account::new(0, 0, &program_id);
    let its_root_account = result.get_account(&its_root_pda).unwrap().clone();

    let ctx = DeployRemoteInterchainTokenContext::new(
        mollusk,
        program_id,
        (payer, payer_account),
        deployer,
        (token_mint_pda, token_mint_account),
        (metadata_account, metadata_account_data),
        (token_manager_pda, modified_token_manager_account),
        (its_root_pda, its_root_account),
        treasury_pda_account,
        gateway_root_pda_account.clone(),
    );

    let checks = vec![Check::err(
        anchor_lang::error::Error::from(
            anchor_lang::error::ErrorCode::AccountDiscriminatorNotFound,
        )
        .into(),
    )];

    let remote_result = deploy_remote_interchain_token_helper(
        salt,
        destination_chain.clone(),
        gas_value,
        ctx,
        checks,
    );

    assert!(remote_result.program_result.is_err());
}

#[test]
fn test_reject_deploy_remote_interchain_token_with_no_metadata() {
    let (setup, _, _, _, _) = setup_test_with_real_signers();

    let init_result = initialize_gateway(&setup);
    assert!(init_result.program_result.is_ok());

    let gas_service_program_id = solana_axelar_gas_service::id();
    let mut mollusk = setup_mollusk(&gas_service_program_id, "solana_axelar_gas_service");

    let (operator, operator_account) = new_test_account();

    let (operator_pda, operator_pda_account) =
        setup_operator(&mut mollusk, operator, &operator_account);

    let (_, treasury_pda_account) = init_gas_service(
        &mollusk,
        operator,
        &operator_account,
        operator_pda,
        &operator_pda_account,
    );

    let (gateway_root_pda, _) = Pubkey::find_program_address(&[GATEWAY_SEED], &GATEWAY_PROGRAM_ID);
    let gateway_root_pda_account = init_result.get_account(&gateway_root_pda).unwrap();

    let program_id = solana_axelar_its::id();
    let mollusk = initialize_mollusk_with_programs();

    let (payer, payer_account) = new_test_account();
    let (deployer, deployer_account) = new_test_account();
    let (operator, operator_account) = new_test_account();

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

    // Create simple token deployment parameters
    let salt = [1u8; 32];
    let name = "Test Token".to_owned();
    let symbol = "TEST".to_owned();
    let decimals = 9u8;
    let initial_supply = 1_000_000_000u64; // 1 billion tokens with 9 decimals

    let ctx = DeployInterchainTokenContext::new(
        mollusk,
        (its_root_pda, its_root_account),
        (deployer, deployer_account),
        (payer, payer_account.clone()),
        None,
        None,
    );

    let (result, token_manager_pda, token_mint_pda, _, _, metadata_account, mollusk) =
        deploy_interchain_token_helper(
            ctx,
            salt,
            name.clone(),
            symbol.clone(),
            decimals,
            initial_supply,
            vec![Check::success()],
        );

    assert!(result.program_result.is_ok());

    let destination_chain = "ethereum".to_owned();
    let gas_value = 0u64;

    let token_mint_account = result.get_account(&token_mint_pda).unwrap().clone();
    let modified_metadata_account_data = Account::new(0, 0, &program_id);
    let token_manager_account = result.get_account(&token_manager_pda).unwrap().clone();
    let its_root_account = result.get_account(&its_root_pda).unwrap().clone();

    let ctx = DeployRemoteInterchainTokenContext::new(
        mollusk,
        program_id,
        (payer, payer_account),
        deployer,
        (token_mint_pda, token_mint_account),
        (metadata_account, modified_metadata_account_data),
        (token_manager_pda, token_manager_account),
        (its_root_pda, its_root_account),
        treasury_pda_account,
        gateway_root_pda_account.clone(),
    );

    let checks = vec![Check::err(
        anchor_lang::error::Error::from(ItsError::InvalidMetaplexDataAccount).into(),
    )];

    let remote_result = deploy_remote_interchain_token_helper(
        salt,
        destination_chain.clone(),
        gas_value,
        ctx,
        checks,
    );

    assert!(remote_result.program_result.is_err());
}

#[test]
fn test_reject_deploy_remote_interchain_token_for_missmatched_mint() {
    let (setup, _, _, _, _) = setup_test_with_real_signers();

    let init_result = initialize_gateway(&setup);
    assert!(init_result.program_result.is_ok());

    let gas_service_program_id = solana_axelar_gas_service::id();
    let mut mollusk = setup_mollusk(&gas_service_program_id, "solana_axelar_gas_service");

    let (operator, operator_account) = new_test_account();

    let (operator_pda, operator_pda_account) =
        setup_operator(&mut mollusk, operator, &operator_account);

    let (_, treasury_pda_account) = init_gas_service(
        &mollusk,
        operator,
        &operator_account,
        operator_pda,
        &operator_pda_account,
    );

    let (gateway_root_pda, _) = Pubkey::find_program_address(&[GATEWAY_SEED], &GATEWAY_PROGRAM_ID);
    let gateway_root_pda_account = init_result.get_account(&gateway_root_pda).unwrap();

    let program_id = solana_axelar_its::id();
    let mollusk = initialize_mollusk_with_programs();

    let (payer, payer_account) = new_test_account();
    let (deployer, deployer_account) = new_test_account();
    let (operator, operator_account) = new_test_account();

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

    // Create simple token deployment parameters
    let salt = [1u8; 32];
    let name = "Test Token".to_owned();
    let symbol = "TEST".to_owned();
    let decimals = 9u8;
    let initial_supply = 1_000_000_000u64; // 1 billion tokens with 9 decimals

    let ctx = DeployInterchainTokenContext::new(
        mollusk,
        (its_root_pda, its_root_account),
        (deployer, deployer_account),
        (payer, payer_account.clone()),
        None,
        None,
    );

    let (result, token_manager_pda, token_mint_pda, _, _, metadata_account, mollusk) =
        deploy_interchain_token_helper(
            ctx,
            salt,
            name.clone(),
            symbol.clone(),
            decimals,
            initial_supply,
            vec![Check::success()],
        );

    assert!(result.program_result.is_ok());

    let destination_chain = "ethereum".to_owned();
    let gas_value = 0u64;

    let token_mint_account = result.get_account(&token_mint_pda).unwrap().clone();
    let modified_metadata_account_data = result.get_account(&metadata_account).unwrap().clone();
    let token_manager_account = result.get_account(&token_manager_pda).unwrap().clone();
    let its_root_account = result.get_account(&its_root_pda).unwrap().clone();

    let invalid_token_id = [123u8; 32];

    let (invalid_token_mint_pda, _) = Pubkey::find_program_address(
        &[
            INTERCHAIN_TOKEN_SEED,
            its_root_pda.as_ref(),
            &invalid_token_id,
        ],
        &program_id,
    );

    let ctx = DeployRemoteInterchainTokenContext::new(
        mollusk,
        program_id,
        (payer, payer_account),
        deployer,
        (invalid_token_mint_pda, token_mint_account),
        (metadata_account, modified_metadata_account_data),
        (token_manager_pda, token_manager_account),
        (its_root_pda, its_root_account),
        treasury_pda_account,
        gateway_root_pda_account.clone(),
    );

    let checks = vec![Check::err(
        anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::ConstraintSeeds).into(),
    )];

    let remote_result = deploy_remote_interchain_token_helper(
        salt,
        destination_chain.clone(),
        gas_value,
        ctx,
        checks,
    );

    assert!(remote_result.program_result.is_err());
}
