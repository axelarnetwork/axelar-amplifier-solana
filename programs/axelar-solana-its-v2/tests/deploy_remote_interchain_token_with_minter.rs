use axelar_solana_gateway_v2::seed_prefixes::GATEWAY_SEED;
use axelar_solana_gateway_v2::ID as GATEWAY_PROGRAM_ID;
use axelar_solana_gateway_v2_test_fixtures::{initialize_gateway, setup_test_with_real_signers};
use axelar_solana_its_v2::seed_prefixes::DEPLOYMENT_APPROVAL_SEED;
use axelar_solana_its_v2_test_fixtures::{
    approve_deploy_remote_interchain_token_helper, deploy_remote_interchain_token_helper,
    ApproveDeployRemoteInterchainTokenContext, DeployRemoteInterchainTokenContext,
};
use axelar_solana_its_v2_test_fixtures::{
    deploy_interchain_token_helper, DeployInterchainTokenContext,
};
use mollusk_test_utils::setup_mollusk;
use solana_sdk::{account::Account, native_token::LAMPORTS_PER_SOL, pubkey::Pubkey};

use crate::initialize::{init_gas_service, init_its_service_with_ethereum_trusted, setup_operator};

#[path = "initialize.rs"]
mod initialize;

#[test]
fn test_deploy_remote_interchain_token_with_minter() {
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

    let mollusk = initialize::initialize_mollusk();

    let payer = Pubkey::new_unique();
    let payer_account = Account::new(10 * LAMPORTS_PER_SOL, 0, &solana_sdk::system_program::ID);

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

    let program_id = axelar_solana_its_v2::id();
    let mollusk = initialize::initialize_mollusk();

    let deployer = Pubkey::new_unique();
    let deployer_account = Account::new(10 * LAMPORTS_PER_SOL, 0, &solana_sdk::system_program::ID);

    // Create simple token deployment parameters
    let salt = [1u8; 32];
    let name = "Test Token".to_string();
    let symbol = "TEST".to_string();
    let decimals = 9u8;
    let initial_supply = 1_000_000_000u64; // 1 billion tokens with 9 decimals

    let minter = Pubkey::new_unique();

    let token_id = axelar_solana_its_v2::utils::interchain_token_id(&deployer, &salt);
    let (token_manager_pda, _) = Pubkey::find_program_address(
        &[
            axelar_solana_its_v2::seed_prefixes::TOKEN_MANAGER_SEED,
            its_root_pda.as_ref(),
            &token_id,
        ],
        &program_id,
    );
    let (minter_roles_pda, _) = Pubkey::find_program_address(
        &[
            axelar_solana_its_v2::state::UserRoles::SEED_PREFIX,
            token_manager_pda.as_ref(),
            minter.as_ref(),
        ],
        &program_id,
    );

    let ctx = DeployInterchainTokenContext::new(
        mollusk,
        its_root_pda,
        its_root_account,
        deployer,
        deployer_account,
        program_id,
        payer,
        payer_account,
        Some(minter),
        Some(minter_roles_pda),
    );

    let (
        deploy_result,
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
        deploy_result.program_result.is_ok(),
        "Deploy interchain token instruction should succeed: {:?}",
        deploy_result.program_result
    );

    // Now test approve deploy remote interchain token
    let destination_chain = "ethereum".to_string();
    let destination_minter = b"0x1234567890abcdef1234567890abcdef12345678".to_vec();

    let destination_chain_hash =
        anchor_lang::solana_program::keccak::hashv(&[destination_chain.as_bytes()]).to_bytes();
    let (deploy_approval_pda, _) = Pubkey::find_program_address(
        &[
            DEPLOYMENT_APPROVAL_SEED,
            minter.as_ref(),
            &token_id,
            &destination_chain_hash,
        ],
        &program_id,
    );

    let ctx = ApproveDeployRemoteInterchainTokenContext::new(
        mollusk,
        deploy_result.clone(),
        minter,
        program_id,
        payer,
        token_manager_pda,
        minter_roles_pda,
        deploy_approval_pda,
    );

    let (approve_result, mollusk) = approve_deploy_remote_interchain_token_helper(
        deployer,
        salt,
        destination_minter.clone(),
        destination_chain.clone(),
        ctx,
    );

    let deploy_approval_pda_account = approve_result.get_account(&deploy_approval_pda).unwrap();
    let minter_roles_pda_account = approve_result.get_account(&minter_roles_pda).unwrap();

    assert!(
        approve_result.program_result.is_ok(),
        "Approve deploy remote interchain token instruction should succeed: {:?}",
        approve_result.program_result
    );

    // Now deploy remote interchain token with remote minter
    let destination_chain = "ethereum".to_string();
    let gas_value = 0u64;

    let ctx = DeployRemoteInterchainTokenContext::new_with_minter(
        deploy_result,
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
        minter,
        deploy_approval_pda,
        deploy_approval_pda_account.clone(),
        minter_roles_pda,
        minter_roles_pda_account.clone(),
    );

    let remote_result =
        deploy_remote_interchain_token_helper(salt, destination_chain.clone(), gas_value, ctx);

    assert!(
        remote_result.program_result.is_ok(),
        "Deploy remote interchain token instruction should succeed: {:?}",
        remote_result.program_result
    );
}
