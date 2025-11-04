#![cfg(test)]
#![allow(clippy::too_many_lines)]

use anchor_lang::AccountDeserialize;
use anchor_lang::InstructionData;
use anchor_lang::ToAccountMetas;
use mollusk_svm::program::keyed_account_for_system_program;
use mollusk_test_utils::get_event_authority_and_program_accounts;
use solana_axelar_its::{
    seed_prefixes::DEPLOYMENT_APPROVAL_SEED, state::deploy_approval::DeployApproval,
};
use solana_axelar_its_test_fixtures::init_its_service;
use solana_axelar_its_test_fixtures::initialize_mollusk;
use solana_axelar_its_test_fixtures::{
    approve_deploy_remote_interchain_token_helper, ApproveDeployRemoteInterchainTokenContext,
};
use solana_axelar_its_test_fixtures::{
    deploy_interchain_token_helper, DeployInterchainTokenContext,
};
use solana_sdk::{
    account::Account, instruction::Instruction, native_token::LAMPORTS_PER_SOL, pubkey::Pubkey,
};

#[test]
fn test_revoke_deploy_remote_interchain_token() {
    let program_id = solana_axelar_its::id();
    let mollusk = initialize_mollusk();

    let payer = Pubkey::new_unique();
    let payer_account = Account::new(10 * LAMPORTS_PER_SOL, 0, &solana_sdk::system_program::ID);

    let deployer = Pubkey::new_unique();
    let deployer_account = Account::new(10 * LAMPORTS_PER_SOL, 0, &solana_sdk::system_program::ID);

    let operator = Pubkey::new_unique();
    let operator_account = Account::new(1_000_000_000, 0, &solana_sdk::system_program::ID);

    let chain_name = "solana".to_owned();
    let its_hub_address = "0x123456789abcdef".to_owned();

    // Initialize ITS service first
    let (
        its_root_pda,
        its_root_account,
        _user_roles_pda,
        _user_roles_account,
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

    // Create simple token deployment parameters
    let salt = [1u8; 32];
    let name = "Test Token".to_owned();
    let symbol = "TEST".to_owned();
    let decimals = 9u8;
    let initial_supply = 1_000_000_000u64; // 1 billion tokens with 9 decimals

    let minter = Pubkey::new_unique();

    let token_id = solana_axelar_its::utils::interchain_token_id(&deployer, &salt);
    let (token_manager_pda, _token_manager_bump) = Pubkey::find_program_address(
        &[
            solana_axelar_its::seed_prefixes::TOKEN_MANAGER_SEED,
            its_root_pda.as_ref(),
            &token_id,
        ],
        &program_id,
    );
    let (minter_roles_pda, _) = Pubkey::find_program_address(
        &[
            solana_axelar_its::state::UserRoles::SEED_PREFIX,
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
    let destination_chain = "ethereum".to_owned();
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
        result,
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

    assert!(
        approve_result.program_result.is_ok(),
        "Approve deploy remote interchain token instruction should succeed: {:?}",
        approve_result.program_result
    );

    // Verify the deploy approval account was created
    let deploy_approval_account = approve_result.get_account(&deploy_approval_pda).unwrap();
    let deploy_approval_data =
        DeployApproval::try_deserialize(&mut deploy_approval_account.data.as_ref()).unwrap();

    // Verify it contains the expected destination minter hash
    let expected_minter_hash =
        anchor_lang::solana_program::keccak::hash(&destination_minter).to_bytes();
    assert_eq!(
        deploy_approval_data.approved_destination_minter,
        expected_minter_hash
    );

    // Now test the revoke instruction
    let (event_authority, event_authority_account, program_account) =
        get_event_authority_and_program_accounts(&program_id);

    let revoke_ix_data = solana_axelar_its::instruction::RevokeDeployRemoteInterchainToken {
        deployer,
        salt,
        destination_chain: destination_chain.clone(),
    }
    .data();

    let revoke_ix = Instruction {
        program_id,
        accounts: solana_axelar_its::accounts::RevokeDeployRemoteInterchainToken {
            payer,
            minter,
            deploy_approval_pda,
            system_program: solana_sdk::system_program::ID,
            // for event CPI
            event_authority,
            program: program_id,
        }
        .to_account_metas(None),
        data: revoke_ix_data,
    };

    // Get updated accounts from the approve result
    let updated_payer_account = approve_result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == payer)
        .map_or_else(
            || Account::new(9 * LAMPORTS_PER_SOL, 0, &solana_sdk::system_program::ID),
            |(_, account)| account.clone(),
        );

    let revoke_accounts = vec![
        (payer, updated_payer_account),
        (minter, Account::new(0, 0, &solana_sdk::system_program::ID)),
        (deploy_approval_pda, deploy_approval_account.clone()),
        keyed_account_for_system_program(),
        // For event CPI
        (event_authority, event_authority_account),
        (program_id, program_account),
    ];

    let revoke_result = mollusk.process_instruction(&revoke_ix, &revoke_accounts);

    assert!(
        revoke_result.program_result.is_ok(),
        "Revoke deploy remote interchain token instruction should succeed: {:?}",
        revoke_result.program_result
    );

    // Verify the deploy approval account was closed (should not exist in resulting accounts)
    let deploy_approval_after_revoke = revoke_result.get_account(&deploy_approval_pda);
    assert!(
        deploy_approval_after_revoke.is_none()
            || deploy_approval_after_revoke.unwrap().data.is_empty(),
        "Deploy approval account should be closed after revoke"
    );

    // Verify that the payer received the rent refund
    let payer_after_revoke = revoke_result.get_account(&payer).unwrap();
    let payer_before_revoke = approve_result.get_account(&payer).unwrap();
    assert!(
        payer_after_revoke.lamports > payer_before_revoke.lamports,
        "Payer should have received rent refund from closed deploy approval account"
    );
}
