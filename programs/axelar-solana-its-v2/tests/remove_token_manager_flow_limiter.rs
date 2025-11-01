use anchor_lang::{
    solana_program::instruction::Instruction, AccountDeserialize, InstructionData, ToAccountMetas,
};
use axelar_solana_its_v2::state::{TokenManager, UserRoles};
use axelar_solana_its_v2_test_fixtures::{
    deploy_interchain_token_helper, init_its_service, initialize_mollusk,
    DeployInterchainTokenContext,
};
use mollusk_svm::program::keyed_account_for_system_program;
use solana_sdk::{account::Account, native_token::LAMPORTS_PER_SOL, pubkey::Pubkey};

#[test]
fn test_add_token_manager_flow_limiter() {
    let program_id = axelar_solana_its_v2::id();
    let mollusk = initialize_mollusk();

    let payer = Pubkey::new_unique();
    let payer_account = Account::new(10 * LAMPORTS_PER_SOL, 0, &solana_sdk::system_program::ID);

    let deployer = Pubkey::new_unique();
    let deployer_account = Account::new(10 * LAMPORTS_PER_SOL, 0, &solana_sdk::system_program::ID);

    let operator = Pubkey::new_unique();
    let operator_account = Account::new(1_000_000_000, 0, &solana_sdk::system_program::ID);

    let chain_name = "solana".to_string();
    let its_hub_address = "0x123456789abcdef".to_string();

    // Initialize ITS service first
    let (its_root_pda, its_root_account, _, _, _, _) = init_its_service(
        &mollusk,
        payer,
        &payer_account,
        payer,
        operator,
        &operator_account,
        chain_name.clone(),
        its_hub_address.clone(),
    );

    // Deploy an interchain token
    let salt = [1u8; 32];
    let name = "Test Token".to_string();
    let symbol = "TEST".to_string();
    let decimals = 9u8;
    let initial_supply = 1_000_000_000u64;

    let minter = Pubkey::new_unique();

    let token_id = axelar_solana_its_v2::utils::interchain_token_id(&deployer, &salt);
    let (token_manager_pda, _token_manager_bump) = TokenManager::find_pda(token_id, its_root_pda);

    // The minter in interchain tokens gets all 3 roles (Operator, Minter, FlowLimiter)
    let (minter_roles_pda, _minter_roles_pda_bump) =
        UserRoles::find_pda(&token_manager_pda, &minter);

    let ctx = DeployInterchainTokenContext::new(
        mollusk,
        its_root_pda,
        its_root_account.clone(),
        deployer,
        deployer_account,
        program_id,
        payer,
        payer_account.clone(),
        Some(minter),
        Some(minter_roles_pda),
    );

    let (deploy_result, token_manager_pda, _, _, _, _, mollusk) =
        deploy_interchain_token_helper(salt, name, symbol, decimals, initial_supply, ctx);

    assert!(
        deploy_result.program_result.is_ok(),
        "Deploy interchain token instruction should succeed: {:?}",
        deploy_result.program_result
    );

    let flow_limiter_user = Pubkey::new_unique();
    let flow_limiter_user_account = Account::new(1_000_000_000, 0, &solana_sdk::system_program::ID);
    let (flow_limiter_roles_pda, _) = UserRoles::find_pda(&token_manager_pda, &flow_limiter_user);

    let add_flow_limiter_ix = Instruction {
        program_id,
        accounts: axelar_solana_its_v2::accounts::AddTokenManagerFlowLimiter {
            system_program: solana_sdk::system_program::ID,
            payer,
            authority_user_account: minter, // use interchain token minter which is also the operator
            authority_roles_account: minter_roles_pda,
            its_root_pda,
            token_manager_pda,
            target_user_account: flow_limiter_user,
            target_roles_account: flow_limiter_roles_pda,
        }
        .to_account_metas(None),
        data: axelar_solana_its_v2::instruction::AddTokenManagerFlowLimiter {}.data(),
    };

    let minter_roles_account = deploy_result
        .get_account(&minter_roles_pda)
        .expect("Minter roles account should exist after token deployment");

    let minter_account = Account::new(1_000_000_000, 0, &solana_sdk::system_program::ID);

    let add_flow_limiter_accounts = vec![
        keyed_account_for_system_program(),
        (payer, payer_account.clone()),
        (minter, minter_account.clone()),
        (minter_roles_pda, minter_roles_account.clone()),
        (its_root_pda, its_root_account.clone()),
        (
            token_manager_pda,
            deploy_result
                .get_account(&token_manager_pda)
                .unwrap()
                .clone(),
        ),
        (flow_limiter_user, flow_limiter_user_account.clone()),
        (
            flow_limiter_roles_pda,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
    ];

    let add_flow_limiter_result =
        mollusk.process_instruction(&add_flow_limiter_ix, &add_flow_limiter_accounts);

    assert!(add_flow_limiter_result.program_result.is_ok());

    // Verify the flow limiter role was added
    let flow_limiter_roles_account = add_flow_limiter_result
        .get_account(&flow_limiter_roles_pda)
        .expect("Flow limiter roles account should exist after adding flow limiter");

    let flow_limiter_roles =
        UserRoles::try_deserialize(&mut flow_limiter_roles_account.data.as_ref())
            .expect("Failed to deserialize flow limiter roles");

    assert!(
        flow_limiter_roles.has_flow_limiter_role(),
        "User should have FLOW_LIMITER role after adding"
    );

    // Now test removing the flow limiter role
    let remove_flow_limiter_ix = Instruction {
        program_id,
        accounts: axelar_solana_its_v2::accounts::RemoveTokenManagerFlowLimiter {
            system_program: solana_sdk::system_program::ID,
            payer,
            authority_user_account: minter, // use interchain token minter which is also the operator
            authority_roles_account: minter_roles_pda,
            its_root_pda,
            token_manager_pda,
            target_user_account: flow_limiter_user,
            target_roles_account: flow_limiter_roles_pda,
        }
        .to_account_metas(None),
        data: axelar_solana_its_v2::instruction::RemoveTokenManagerFlowLimiter {}.data(),
    };

    let remove_flow_limiter_accounts = vec![
        keyed_account_for_system_program(),
        (payer, payer_account.clone()),
        (minter, minter_account.clone()),
        (minter_roles_pda, minter_roles_account.clone()),
        (its_root_pda, its_root_account.clone()),
        (
            token_manager_pda,
            add_flow_limiter_result
                .get_account(&token_manager_pda)
                .unwrap()
                .clone(),
        ),
        (flow_limiter_user, flow_limiter_user_account.clone()),
        (flow_limiter_roles_pda, flow_limiter_roles_account.clone()),
    ];

    let remove_flow_limiter_result =
        mollusk.process_instruction(&remove_flow_limiter_ix, &remove_flow_limiter_accounts);

    assert!(remove_flow_limiter_result.program_result.is_ok());

    // Verify that the flow limiter role was removed correctly
    let updated_flow_limiter_roles_account = remove_flow_limiter_result
        .get_account(&flow_limiter_roles_pda)
        .expect("Flow limiter roles account should exist after removing flow limiter");

    let updated_flow_limiter_roles =
        UserRoles::try_deserialize(&mut updated_flow_limiter_roles_account.data.as_ref())
            .expect("Failed to deserialize updated flow limiter roles");

    assert!(
        !updated_flow_limiter_roles.has_flow_limiter_role(),
        "User should not have FLOW_LIMITER role after removal"
    );

    // Verify that no other roles were affected
    assert!(
        !updated_flow_limiter_roles.has_operator_role(),
        "User should not have OPERATOR role"
    );
    assert!(
        !updated_flow_limiter_roles.has_minter_role(),
        "User should not have MINTER role"
    );
}
