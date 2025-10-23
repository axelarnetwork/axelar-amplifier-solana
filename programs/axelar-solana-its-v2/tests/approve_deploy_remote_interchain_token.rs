use anchor_lang::AccountDeserialize;
use axelar_solana_its_v2::{
    seed_prefixes::DEPLOYMENT_APPROVAL_SEED, state::deploy_approval::DeployApproval,
};
use axelar_solana_its_v2_test_fixtures::{
    approve_deploy_remote_interchain_token_helper, ApproveDeployRemoteInterchainTokenContext,
};
use axelar_solana_its_v2_test_fixtures::{
    deploy_interchain_token_helper, DeployInterchainTokenContext,
};
use solana_sdk::{account::Account, native_token::LAMPORTS_PER_SOL, pubkey::Pubkey};

#[path = "initialize.rs"]
mod initialize;

#[test]
fn test_approve_deploy_remote_interchain_token() {
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

    // Initialize ITS service first
    let (
        its_root_pda,
        its_root_account,
        _user_roles_pda,
        _user_roles_account,
        _program_data,
        _program_data_account,
    ) = initialize::init_its_service(
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

    let minter = Pubkey::new_unique();

    let token_id = axelar_solana_its_v2::utils::interchain_token_id(&deployer, &salt);
    let (token_manager_pda, _token_manager_bump) = Pubkey::find_program_address(
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

    let (result, token_manager_pda, _, _, _, _, mollusk) = deploy_interchain_token_helper(
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

    // Now test approve deploy remote interchain token
    let destination_chain = "ethereum".to_string();
    let destination_minter = b"0x1234567890abcdef1234567890abcdef12345678".to_vec();

    let destination_chain_hash =
        anchor_lang::solana_program::keccak::hashv(&[destination_chain.as_bytes()]).to_bytes();
    let (deploy_approval_pda, deploy_approval_bump) = Pubkey::find_program_address(
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
        result,
        minter,
        program_id,
        payer,
        token_manager_pda,
        minter_roles_pda,
        deploy_approval_pda,
    );

    let (approve_result, _) = approve_deploy_remote_interchain_token_helper(
        deployer,
        salt,
        destination_minter.clone(),
        destination_chain.clone(),
        ctx,
    );

    assert!(
        approve_result.program_result.is_ok(),
        "Approve deploy remote interchain token instruction should succeed: {:?}",
        approve_result.program_result
    );

    let deploy_approval_account = approve_result.get_account(&deploy_approval_pda).unwrap();
    let deploy_approval =
        DeployApproval::try_deserialize(&mut deploy_approval_account.data.as_ref()).unwrap();

    let expected_destination_minter_hash =
        anchor_lang::solana_program::keccak::hash(&destination_minter).to_bytes();
    assert_eq!(
        deploy_approval.approved_destination_minter, expected_destination_minter_hash,
        "Approved destination minter hash should match"
    );
    assert_eq!(
        deploy_approval.bump, deploy_approval_bump,
        "Deploy approval bump should match"
    );
}
