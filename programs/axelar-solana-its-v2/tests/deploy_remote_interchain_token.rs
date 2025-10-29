use crate::initialize::init_gas_service;
use crate::initialize::init_its_service_with_ethereum_trusted;
use crate::initialize::setup_operator;
use axelar_solana_gateway_v2::seed_prefixes::GATEWAY_SEED;
use axelar_solana_gateway_v2::ID as GATEWAY_PROGRAM_ID;
use axelar_solana_gateway_v2_test_fixtures::initialize_gateway;
use axelar_solana_gateway_v2_test_fixtures::setup_test_with_real_signers;
use axelar_solana_its_v2_test_fixtures::deploy_remote_interchain_token_helper;
use axelar_solana_its_v2_test_fixtures::DeployRemoteInterchainTokenContext;
use axelar_solana_its_v2_test_fixtures::{
    deploy_interchain_token_helper, DeployInterchainTokenContext,
};
use mollusk_test_utils::setup_mollusk;
use solana_sdk::{account::Account, native_token::LAMPORTS_PER_SOL, pubkey::Pubkey};

#[test]
fn test_deploy_remote_interchain_token() {
    // Initialize gateway
    let (setup, _, _, _, _) = setup_test_with_real_signers();

    let init_result = initialize_gateway(&setup);
    assert!(init_result.program_result.is_ok());

    // Initialize gas service
    let gas_service_program_id = axelar_solana_gas_service_v2::id();
    let mut mollusk = setup_mollusk(&gas_service_program_id, "axelar_solana_gas_service_v2");

    let operator = Pubkey::new_unique();
    let operator_account = Account::new(1_000_000_000, 0, &solana_sdk::system_program::ID);

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
    let mollusk = initialize::initialize_mollusk();

    let payer = Pubkey::new_unique();
    let payer_account = Account::new(10 * LAMPORTS_PER_SOL, 0, &solana_sdk::system_program::ID);

    let deployer = Pubkey::new_unique();
    let deployer_account = Account::new(10 * LAMPORTS_PER_SOL, 0, &solana_sdk::system_program::ID);

    let operator = Pubkey::new_unique();
    let operator_account = Account::new(1_000_000_000, 0, &solana_sdk::system_program::ID);

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

    let ctx = DeployInterchainTokenContext::new(
        mollusk,
        its_root_pda,
        its_root_account,
        deployer,
        deployer_account,
        program_id,
        payer,
        payer_account,
        None,
        None,
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
        ctx,
    );

    assert!(
        result.program_result.is_ok(),
        "Deploy interchain token instruction should succeed: {:?}",
        result.program_result
    );

    let destination_chain = "ethereum".to_string();
    let gas_value = 0u64;

    let ctx = DeployRemoteInterchainTokenContext::new(
        result,
        mollusk,
        program_id,
        payer,
        deployer,
        token_mint_pda,
        metadata_account,
        token_manager_pda,
        its_root_pda,
        treasury_pda,
        gateway_root_pda_account.clone(),
    );

    let remote_result =
        deploy_remote_interchain_token_helper(salt, destination_chain.clone(), gas_value, ctx);

    assert!(
        remote_result.program_result.is_ok(),
        "Deploy remote interchain token instruction should succeed: {:?}",
        remote_result.program_result
    );
}
