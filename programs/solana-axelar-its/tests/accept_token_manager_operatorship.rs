use anchor_lang::AccountDeserialize;
use axelar_solana_its_v2::state::{RoleProposal, Roles, TokenManager, UserRoles};
use axelar_solana_its_v2::utils::interchain_token_id;
use axelar_solana_its_v2_test_fixtures::{
    deploy_interchain_token_helper, init_its_service, initialize_mollusk,
    DeployInterchainTokenContext,
};
use mollusk_svm::program::keyed_account_for_system_program;
use {
    anchor_lang::{solana_program::instruction::Instruction, InstructionData, ToAccountMetas},
    solana_sdk::{account::Account, native_token::LAMPORTS_PER_SOL, pubkey::Pubkey},
};

#[test]
fn test_accept_token_manager_operatorship() {
    let program_id = axelar_solana_its_v2::id();
    let mollusk = initialize_mollusk();

    let upgrade_authority = Pubkey::new_unique();
    let payer = upgrade_authority;
    let payer_account = Account::new(10 * LAMPORTS_PER_SOL, 0, &solana_sdk::system_program::ID);

    let current_operator = Pubkey::new_unique();
    let current_operator_account =
        Account::new(10 * LAMPORTS_PER_SOL, 0, &solana_sdk::system_program::ID);

    let proposed_operator = Pubkey::new_unique();
    let proposed_operator_account =
        Account::new(10 * LAMPORTS_PER_SOL, 0, &solana_sdk::system_program::ID);

    let chain_name = "solana".to_string();
    let its_hub_address = "0x123456789abcdef".to_string();

    let (its_root_pda, its_root_account, _, _, _, _) = init_its_service(
        &mollusk,
        payer,
        &payer_account,
        upgrade_authority,
        current_operator,
        &current_operator_account,
        chain_name.clone(),
        its_hub_address.clone(),
    );

    // Deploy an interchain token to create a TokenManager PDA
    let salt = [1u8; 32];
    let token_name = "Test Token".to_string();
    let token_symbol = "TEST".to_string();
    let decimals = 9u8;
    let initial_supply = 1_000_000_000u64;

    let token_id = interchain_token_id(&current_operator, &salt);
    let (token_manager_pda, _) = TokenManager::find_pda(token_id, its_root_pda);
    let (minter_roles_pda, _) = UserRoles::find_pda(&token_manager_pda, &current_operator);

    let deploy_ctx = DeployInterchainTokenContext::new(
        mollusk,
        its_root_pda,
        its_root_account.clone(),
        current_operator,
        current_operator_account.clone(),
        program_id,
        payer,
        payer_account.clone(),
        Some(current_operator),
        Some(minter_roles_pda),
    );

    let (deploy_result, token_manager_pda, _, _, _, _, mollusk) = deploy_interchain_token_helper(
        salt,
        token_name,
        token_symbol,
        decimals,
        initial_supply,
        deploy_ctx,
    );

    assert!(deploy_result.program_result.is_ok());

    let token_manager_account = deploy_result
        .get_account(&token_manager_pda)
        .expect("TokenManager account should exist");

    let current_operator_token_roles_account = deploy_result
        .get_account(&minter_roles_pda)
        .expect("Current operator token roles account should exist");

    let current_operator_token_roles =
        UserRoles::try_deserialize(&mut current_operator_token_roles_account.data.as_slice())
            .expect("Failed to deserialize current operator token roles");

    assert!(current_operator_token_roles.roles.contains(Roles::OPERATOR));

    // Propose operatorship transfer
    let (proposal_pda, _bump) = RoleProposal::find_pda(
        &token_manager_pda,
        &current_operator,
        &proposed_operator,
        &program_id,
    );

    let propose_ix = Instruction {
        program_id,
        accounts: axelar_solana_its_v2::accounts::ProposeTokenManagerOperatorship {
            system_program: solana_sdk::system_program::ID,
            payer,
            origin_user_account: current_operator,
            origin_roles_account: minter_roles_pda,
            its_root_pda,
            token_manager_account: token_manager_pda,
            destination_user_account: proposed_operator,
            proposal_account: proposal_pda,
        }
        .to_account_metas(None),
        data: axelar_solana_its_v2::instruction::ProposeTokenManagerOperatorship {}.data(),
    };

    let propose_accounts = vec![
        keyed_account_for_system_program(),
        (payer, payer_account.clone()),
        (current_operator, current_operator_account.clone()),
        (
            minter_roles_pda,
            current_operator_token_roles_account.clone(),
        ),
        (its_root_pda, its_root_account.clone()),
        (token_manager_pda, token_manager_account.clone()),
        (proposed_operator, proposed_operator_account.clone()),
        (
            proposal_pda,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
    ];

    let propose_result = mollusk.process_instruction(&propose_ix, &propose_accounts);
    assert!(propose_result.program_result.is_ok());

    // Verify proposal was created
    let proposal_account = propose_result
        .get_account(&proposal_pda)
        .expect("Proposal account should exist");
    let proposal_data = RoleProposal::try_deserialize(&mut proposal_account.data.as_slice())
        .expect("Failed to deserialize RoleProposal");
    assert_eq!(proposal_data.roles, Roles::OPERATOR);

    // Accept operatorship transfer
    let (new_operator_roles_pda, new_operator_roles_bump) =
        UserRoles::find_pda(&token_manager_pda, &proposed_operator);

    let accept_ix = Instruction {
        program_id,
        accounts: axelar_solana_its_v2::accounts::AcceptTokenManagerOperatorship {
            system_program: solana_sdk::system_program::ID,
            payer,
            destination_user_account: proposed_operator,
            destination_roles_account: new_operator_roles_pda,
            its_root_pda,
            token_manager_account: token_manager_pda,
            origin_user_account: current_operator,
            origin_roles_account: minter_roles_pda,
            proposal_account: proposal_pda,
        }
        .to_account_metas(None),
        data: axelar_solana_its_v2::instruction::AcceptTokenManagerOperatorship {}.data(),
    };

    let accept_accounts = vec![
        keyed_account_for_system_program(),
        (payer, payer_account.clone()),
        (proposed_operator, proposed_operator_account.clone()),
        (
            new_operator_roles_pda,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
        (its_root_pda, its_root_account.clone()),
        (token_manager_pda, token_manager_account.clone()),
        (current_operator, current_operator_account.clone()),
        (
            minter_roles_pda,
            propose_result
                .get_account(&minter_roles_pda)
                .unwrap()
                .clone(),
        ),
        (proposal_pda, proposal_account.clone()),
    ];

    let accept_result = mollusk.process_instruction(&accept_ix, &accept_accounts);
    assert!(accept_result.program_result.is_ok());

    // Old operator should no longer have OPERATOR role
    let old_operator_roles_account = accept_result
        .get_account(&minter_roles_pda)
        .expect("Old operator roles account should exist");

    let old_operator_roles =
        UserRoles::try_deserialize(&mut old_operator_roles_account.data.as_slice())
            .expect("Failed to deserialize old operator roles");

    assert!(!old_operator_roles.roles.contains(Roles::OPERATOR));

    // New operator should have OPERATOR role
    let new_operator_roles_account = accept_result
        .get_account(&new_operator_roles_pda)
        .expect("New operator roles account should exist");
    let new_operator_roles =
        UserRoles::try_deserialize(&mut new_operator_roles_account.data.as_slice())
            .expect("Failed to deserialize new operator roles");

    assert!(new_operator_roles.roles.contains(Roles::OPERATOR));
    assert_eq!(new_operator_roles.bump, new_operator_roles_bump);

    // Proposal account should be closed
    let proposal_pda_account = accept_result.get_account(&proposal_pda).unwrap();
    assert!(proposal_pda_account.data.len() == 0);
}
