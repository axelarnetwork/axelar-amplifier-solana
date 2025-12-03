#![cfg(test)]
#![allow(clippy::too_many_lines)]

use anchor_lang::AccountDeserialize;
use anchor_spl::token_2022::spl_token_2022;
use mollusk_svm::result::Check;
use solana_axelar_its::{
    state::{Roles, Type, UserRoles},
    ItsError,
};
use solana_axelar_its_test_fixtures::{
    create_test_mint, execute_register_custom_token_helper, handover_mint_authority_helper,
    init_its_service, initialize_mollusk_with_programs, new_test_account,
    HandoverMintAuthorityContext, RegisterCustomTokenContext, RegisterCustomTokenParams,
};
use solana_program::program_pack::Pack;
use solana_sdk::pubkey::Pubkey;

#[test]
fn handover_mint_authority_success() {
    let mollusk = initialize_mollusk_with_programs();

    let (payer, payer_account) = new_test_account();
    let (deployer, deployer_account) = new_test_account();
    let (operator, operator_account) = new_test_account();
    let (mint_authority, mint_authority_account) = new_test_account();

    let chain_name = "solana".to_owned();
    let its_hub_address = "0x123456789abcdef".to_owned();

    // Initialize ITS service
    let (its_root_pda, its_root_account, _, _, _, _) = init_its_service(
        &mollusk,
        payer,
        &payer_account,
        payer,
        operator,
        &operator_account,
        chain_name,
        its_hub_address,
    );

    let (token_mint, token_mint_account) = create_test_mint(mint_authority);

    // Register the token as a custom token with MintBurn type
    let salt = [1u8; 32];
    let token_manager_type = Type::MintBurn;

    let ctx = RegisterCustomTokenContext {
        mollusk,
        payer: (payer, payer_account.clone()),
        deployer: (deployer, deployer_account.clone()),
        its_root: (its_root_pda, its_root_account.clone()),
        token_mint: (token_mint, token_mint_account.clone()),
    };

    let params = RegisterCustomTokenParams {
        salt,
        token_manager_type,
        operator: None,
    };

    let register_result = execute_register_custom_token_helper(ctx, params, vec![Check::success()]);
    assert!(register_result.result.program_result.is_ok());

    // Calculate token ID
    let token_id = {
        let deploy_salt = solana_axelar_its::utils::linked_token_deployer_salt(&deployer, &salt);
        solana_axelar_its::utils::interchain_token_id_internal(&deploy_salt)
    };

    // Get the updated accounts from the register result
    let updated_its_root_account = register_result
        .result
        .get_account(&its_root_pda)
        .unwrap()
        .clone();
    let token_manager_account = register_result
        .result
        .get_account(&register_result.token_manager_pda)
        .unwrap()
        .clone();

    // Test handover mint authority using the fixture
    let handover_ctx = HandoverMintAuthorityContext::new(
        register_result.mollusk,
        (payer, payer_account),
        (mint_authority, mint_authority_account),
        (token_mint, token_mint_account),
        (its_root_pda, updated_its_root_account),
        (register_result.token_manager_pda, token_manager_account),
        token_id,
    );

    let (result, _) = handover_mint_authority_helper(handover_ctx, vec![Check::success()]);

    assert!(result.program_result.is_ok());

    // Verify that the mint authority was transferred to the token manager
    let updated_mint_account = result.get_account(&token_mint).unwrap();
    let mint_data = &updated_mint_account.data;
    let mint_state = spl_token_2022::state::Mint::unpack(mint_data).unwrap();

    assert_eq!(
        mint_state.mint_authority,
        Some(register_result.token_manager_pda).into()
    );

    // Verify that the authority received the MINTER role
    let (minter_roles_pda, _) =
        UserRoles::find_pda(&register_result.token_manager_pda, &mint_authority);
    let minter_roles_account = result.get_account(&minter_roles_pda).unwrap();
    let user_roles = UserRoles::try_deserialize(&mut minter_roles_account.data.as_ref()).unwrap();

    assert!(user_roles.roles.contains(Roles::MINTER));
}

#[test]
fn handover_mint_authority_wrong_token_id() {
    let mollusk = initialize_mollusk_with_programs();

    let (payer, payer_account) = new_test_account();
    let (deployer, deployer_account) = new_test_account();
    let (operator, operator_account) = new_test_account();
    let (mint_authority, mint_authority_account) = new_test_account();

    let chain_name = "solana".to_owned();
    let its_hub_address = "0x123456789abcdef".to_owned();

    // Initialize ITS service
    let (its_root_pda, its_root_account, _, _, _, _) = init_its_service(
        &mollusk,
        payer,
        &payer_account,
        payer,
        operator,
        &operator_account,
        chain_name,
        its_hub_address,
    );

    let (token_mint, token_mint_account) = create_test_mint(mint_authority);

    // Register the token as a custom token with MintBurn type
    let salt = [1u8; 32];
    let token_manager_type = Type::MintBurn;

    let ctx = RegisterCustomTokenContext {
        mollusk,
        payer: (payer, payer_account.clone()),
        deployer: (deployer, deployer_account.clone()),
        its_root: (its_root_pda, its_root_account.clone()),
        token_mint: (token_mint, token_mint_account.clone()),
    };

    let params = RegisterCustomTokenParams {
        salt,
        token_manager_type,
        operator: None,
    };

    let register_result = execute_register_custom_token_helper(ctx, params, vec![Check::success()]);
    assert!(register_result.result.program_result.is_ok());

    // Use wrong token ID
    let wrong_token_id = [99u8; 32];

    // Get the updated accounts from the register result
    let updated_its_root_account = register_result
        .result
        .get_account(&its_root_pda)
        .unwrap()
        .clone();
    let token_manager_account = register_result
        .result
        .get_account(&register_result.token_manager_pda)
        .unwrap()
        .clone();

    // Test handover mint authority with wrong token ID
    let handover_ctx = HandoverMintAuthorityContext::new(
        register_result.mollusk,
        (payer, payer_account),
        (mint_authority, mint_authority_account),
        (token_mint, token_mint_account),
        (its_root_pda, updated_its_root_account),
        (register_result.token_manager_pda, token_manager_account),
        wrong_token_id,
    );

    let checks = vec![Check::err(
        anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::ConstraintSeeds).into(),
    )];

    let (result, _) = handover_mint_authority_helper(handover_ctx, checks);

    assert!(result.program_result.is_err());
}

#[test]
fn handover_mint_authority_not_mint_authority() {
    let mollusk = initialize_mollusk_with_programs();

    let (payer, payer_account) = new_test_account();
    let (deployer, deployer_account) = new_test_account();
    let (operator, operator_account) = new_test_account();
    let (mint_authority, _) = new_test_account();
    let (fake_authority, fake_authority_account) = new_test_account();

    let chain_name = "solana".to_owned();
    let its_hub_address = "0x123456789abcdef".to_owned();

    // Initialize ITS service
    let (its_root_pda, its_root_account, _, _, _, _) = init_its_service(
        &mollusk,
        payer,
        &payer_account,
        payer,
        operator,
        &operator_account,
        chain_name,
        its_hub_address,
    );

    let (token_mint, token_mint_account) = create_test_mint(mint_authority);

    // Register the token as a custom token with MintBurn type
    let salt = [1u8; 32];
    let token_manager_type = Type::MintBurn;

    let ctx = RegisterCustomTokenContext {
        mollusk,
        payer: (payer, payer_account.clone()),
        deployer: (deployer, deployer_account.clone()),
        its_root: (its_root_pda, its_root_account.clone()),
        token_mint: (token_mint, token_mint_account.clone()),
    };

    let params = RegisterCustomTokenParams {
        salt,
        token_manager_type,
        operator: None,
    };

    let register_result = execute_register_custom_token_helper(ctx, params, vec![Check::success()]);
    assert!(register_result.result.program_result.is_ok());

    // Calculate correct token ID
    let token_id = {
        let deploy_salt = solana_axelar_its::utils::linked_token_deployer_salt(&deployer, &salt);
        solana_axelar_its::utils::interchain_token_id_internal(&deploy_salt)
    };

    // Get the updated accounts from the register result
    let updated_its_root_account = register_result
        .result
        .get_account(&its_root_pda)
        .unwrap()
        .clone();
    let token_manager_account = register_result
        .result
        .get_account(&register_result.token_manager_pda)
        .unwrap()
        .clone();

    // Test handover mint authority with fake authority (not the actual mint authority)
    let handover_ctx = HandoverMintAuthorityContext::new(
        register_result.mollusk,
        (payer, payer_account),
        (fake_authority, fake_authority_account), // Using fake authority instead of mint_authority
        (token_mint, token_mint_account),
        (its_root_pda, updated_its_root_account),
        (register_result.token_manager_pda, token_manager_account),
        token_id,
    );

    let checks = vec![Check::err(
        anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::ConstraintMintMintAuthority)
            .into(),
    )];

    // This should fail because fake_authority is not the mint authority
    let (result, _) = handover_mint_authority_helper(handover_ctx, checks);

    assert!(result.program_result.is_err());
}

#[test]
fn handover_mint_authority_wrong_token_manager() {
    let mollusk = initialize_mollusk_with_programs();

    let (payer, payer_account) = new_test_account();
    let (deployer, deployer_account) = new_test_account();
    let (operator, operator_account) = new_test_account();
    let (mint_authority, mint_authority_account) = new_test_account();

    let chain_name = "solana".to_owned();
    let its_hub_address = "0x123456789abcdef".to_owned();

    // Initialize ITS service
    let (its_root_pda, its_root_account, _, _, _, _) = init_its_service(
        &mollusk,
        payer,
        &payer_account,
        payer,
        operator,
        &operator_account,
        chain_name,
        its_hub_address,
    );

    let (token_mint, token_mint_account) = create_test_mint(mint_authority);

    // Register the token as a custom token with MintBurn type
    let salt = [1u8; 32];
    let token_manager_type = Type::MintBurn;

    let ctx = RegisterCustomTokenContext {
        mollusk,
        payer: (payer, payer_account.clone()),
        deployer: (deployer, deployer_account.clone()),
        its_root: (its_root_pda, its_root_account.clone()),
        token_mint: (token_mint, token_mint_account.clone()),
    };

    let params = RegisterCustomTokenParams {
        salt,
        token_manager_type,
        operator: None,
    };

    let register_result = execute_register_custom_token_helper(ctx, params, vec![Check::success()]);
    assert!(register_result.result.program_result.is_ok());

    // Calculate correct token ID
    let token_id = {
        let deploy_salt = solana_axelar_its::utils::linked_token_deployer_salt(&deployer, &salt);
        solana_axelar_its::utils::interchain_token_id_internal(&deploy_salt)
    };

    // Get the updated accounts from the register result
    let updated_its_root_account = register_result
        .result
        .get_account(&its_root_pda)
        .unwrap()
        .clone();
    let token_manager_account = register_result
        .result
        .get_account(&register_result.token_manager_pda)
        .unwrap()
        .clone();

    // Create a fake token manager PDA
    let fake_token_manager = Pubkey::new_unique();

    // Test handover mint authority with wrong token manager
    let handover_ctx = HandoverMintAuthorityContext::new(
        register_result.mollusk,
        (payer, payer_account),
        (mint_authority, mint_authority_account),
        (token_mint, token_mint_account),
        (its_root_pda, updated_its_root_account),
        (fake_token_manager, token_manager_account), // Wrong token manager PDA
        token_id,
    );

    let checks = vec![Check::err(
        anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::ConstraintSeeds).into(),
    )];

    let (result, _) = handover_mint_authority_helper(handover_ctx, checks);

    assert!(result.program_result.is_err());
}

#[test]
fn handover_mint_authority_non_mint_burn_token() {
    let mollusk = initialize_mollusk_with_programs();

    let (payer, payer_account) = new_test_account();
    let (deployer, deployer_account) = new_test_account();
    let (operator, operator_account) = new_test_account();
    let (mint_authority, mint_authority_account) = new_test_account();

    let chain_name = "solana".to_owned();
    let its_hub_address = "0x123456789abcdef".to_owned();

    // Initialize ITS service
    let (its_root_pda, its_root_account, _, _, _, _) = init_its_service(
        &mollusk,
        payer,
        &payer_account,
        payer,
        operator,
        &operator_account,
        chain_name,
        its_hub_address,
    );

    let (token_mint, token_mint_account) = create_test_mint(mint_authority);

    // Register the token as a custom token with LockUnlock type (not MintBurn)
    let salt = [1u8; 32];
    let token_manager_type = Type::LockUnlock; // This should fail for handover

    let ctx = RegisterCustomTokenContext {
        mollusk,
        payer: (payer, payer_account.clone()),
        deployer: (deployer, deployer_account.clone()),
        its_root: (its_root_pda, its_root_account.clone()),
        token_mint: (token_mint, token_mint_account.clone()),
    };

    let params = RegisterCustomTokenParams {
        salt,
        token_manager_type,
        operator: None,
    };

    let register_result = execute_register_custom_token_helper(ctx, params, vec![Check::success()]);
    assert!(register_result.result.program_result.is_ok());

    // Calculate correct token ID
    let token_id = {
        let deploy_salt = solana_axelar_its::utils::linked_token_deployer_salt(&deployer, &salt);
        solana_axelar_its::utils::interchain_token_id_internal(&deploy_salt)
    };

    // Get the updated accounts from the register result
    let updated_its_root_account = register_result
        .result
        .get_account(&its_root_pda)
        .unwrap()
        .clone();
    let token_manager_account = register_result
        .result
        .get_account(&register_result.token_manager_pda)
        .unwrap()
        .clone();

    // Test handover mint authority on non-MintBurn token
    let handover_ctx = HandoverMintAuthorityContext::new(
        register_result.mollusk,
        (payer, payer_account),
        (mint_authority, mint_authority_account),
        (token_mint, token_mint_account),
        (its_root_pda, updated_its_root_account),
        (register_result.token_manager_pda, token_manager_account),
        token_id,
    );

    let checks = vec![Check::err(
        anchor_lang::error::Error::from(ItsError::InvalidTokenManagerType).into(),
    )];

    let (result, _) = handover_mint_authority_helper(handover_ctx, checks);

    assert!(result.program_result.is_err());
}
