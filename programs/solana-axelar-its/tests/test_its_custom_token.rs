#![cfg(test)]
#![allow(clippy::indexing_slicing)]

use anchor_lang::InstructionData;
use anchor_spl::token_2022::spl_token_2022;
use mollusk_harness::{ItsTestHarness, TestHarness};
use mollusk_svm::result::Check;
use solana_axelar_its::{
    instructions::{
        make_handover_mint_authority_instruction, make_register_custom_token_instruction,
    },
    state::{roles, token_manager::Type, InterchainTokenService, TokenManager, UserRoles},
    utils::{interchain_token_id_internal, linked_token_deployer_salt},
    ItsError,
};
use solana_sdk::program_pack::Pack;

/// Registers a custom token via the harness and returns the token_id.
fn register_custom_token(
    harness: &ItsTestHarness,
    deployer: solana_sdk::pubkey::Pubkey,
    token_mint: solana_sdk::pubkey::Pubkey,
    salt: [u8; 32],
    token_manager_type: Type,
    operator: Option<solana_sdk::pubkey::Pubkey>,
) -> [u8; 32] {
    let (ix, _) = make_register_custom_token_instruction(
        harness.payer,
        deployer,
        token_mint,
        spl_token_2022::ID,
        salt,
        token_manager_type,
        operator,
    );

    harness
        .ctx
        .process_and_validate_instruction(&ix, &[Check::success()]);

    let deploy_salt = linked_token_deployer_salt(&deployer, &salt);
    interchain_token_id_internal(&deploy_salt)
}

// ── Register Custom Token ────────────────────────────────────────────

#[test]
fn register_custom_token_without_operator() {
    let harness = ItsTestHarness::new();

    let mint_authority = harness.get_new_wallet();
    let token_mint = harness.create_spl_token_mint(mint_authority, 9, None);
    let deployer = harness.get_new_wallet();
    let salt = [1u8; 32];

    let token_id =
        register_custom_token(&harness, deployer, token_mint, salt, Type::MintBurn, None);

    let its_root_pda = InterchainTokenService::find_pda().0;
    let token_manager_pda = TokenManager::find_pda(token_id, its_root_pda).0;
    let tm: TokenManager = harness
        .get_account_as(&token_manager_pda)
        .expect("token manager should exist");

    assert_eq!(tm.ty, Type::MintBurn);
    assert_eq!(tm.token_address, token_mint);
}

#[test]
fn register_custom_token_with_operator() {
    let harness = ItsTestHarness::new();

    let mint_authority = harness.get_new_wallet();
    let token_mint = harness.create_spl_token_mint(mint_authority, 9, None);
    let deployer = harness.get_new_wallet();
    let op = harness.get_new_wallet();
    let salt = [2u8; 32];

    let token_id = register_custom_token(
        &harness,
        deployer,
        token_mint,
        salt,
        Type::LockUnlock,
        Some(op),
    );

    let its_root_pda = InterchainTokenService::find_pda().0;
    let token_manager_pda = TokenManager::find_pda(token_id, its_root_pda).0;

    let tm: TokenManager = harness
        .get_account_as(&token_manager_pda)
        .expect("token manager should exist");
    assert_eq!(tm.ty, Type::LockUnlock);

    let op_roles_pda = UserRoles::find_pda(&token_manager_pda, &op).0;
    let user_roles: UserRoles = harness
        .get_account_as(&op_roles_pda)
        .expect("operator roles should exist");
    assert!(user_roles.has_operator_role());
}

#[test]
fn reject_register_custom_token_with_native_interchain_type() {
    let harness = ItsTestHarness::new();

    let mint_authority = harness.get_new_wallet();
    let token_mint = harness.create_spl_token_mint(mint_authority, 9, None);
    let deployer = harness.get_new_wallet();
    let salt = [3u8; 32];

    let (ix, _) = make_register_custom_token_instruction(
        harness.payer,
        deployer,
        token_mint,
        spl_token_2022::ID,
        salt,
        Type::NativeInterchainToken,
        None,
    );

    harness.ctx.process_and_validate_instruction(
        &ix,
        &[Check::err(ItsError::InvalidInstructionData.into())],
    );
}

#[test]
fn reject_register_custom_token_with_mismatched_operator() {
    let harness = ItsTestHarness::new();

    let mint_authority = harness.get_new_wallet();
    let token_mint = harness.create_spl_token_mint(mint_authority, 9, None);
    let deployer = harness.get_new_wallet();
    let op = harness.get_new_wallet();
    let wrong_op = harness.get_new_wallet();
    let salt = [4u8; 32];

    // Build instruction with `op` as the account but `wrong_op` in instruction data
    let (mut ix, _) = make_register_custom_token_instruction(
        harness.payer,
        deployer,
        token_mint,
        spl_token_2022::ID,
        salt,
        Type::LockUnlock,
        Some(op),
    );

    // Override instruction data with mismatched operator
    ix.data = solana_axelar_its::instruction::RegisterCustomToken {
        salt,
        token_manager_type: Type::LockUnlock,
        operator: Some(wrong_op),
    }
    .data();

    harness
        .ctx
        .process_and_validate_instruction(&ix, &[Check::err(ItsError::InvalidArgument.into())]);
}

// ── Handover Mint Authority ──────────────────────────────────────────

#[test]
fn handover_mint_authority_success() {
    let harness = ItsTestHarness::new();

    let mint_authority = harness.get_new_wallet();
    let token_mint = harness.create_spl_token_mint(mint_authority, 9, None);
    let deployer = harness.get_new_wallet();
    let salt = [10u8; 32];

    let token_id =
        register_custom_token(&harness, deployer, token_mint, salt, Type::MintBurn, None);

    let (ix, _) = make_handover_mint_authority_instruction(
        harness.payer,
        mint_authority,
        token_mint,
        spl_token_2022::ID,
        token_id,
    );

    harness
        .ctx
        .process_and_validate_instruction(&ix, &[Check::success()]);

    // Verify mint authority transferred to token manager
    let its_root_pda = InterchainTokenService::find_pda().0;
    let token_manager_pda = TokenManager::find_pda(token_id, its_root_pda).0;

    let mint_account = harness.get_account(&token_mint).expect("mint should exist");
    let mint_state = spl_token_2022::state::Mint::unpack(&mint_account.data).expect("valid mint");
    assert_eq!(mint_state.mint_authority, Some(token_manager_pda).into());

    // Verify authority received MINTER role
    let minter_roles_pda = UserRoles::find_pda(&token_manager_pda, &mint_authority).0;
    let user_roles: UserRoles = harness
        .get_account_as(&minter_roles_pda)
        .expect("minter roles should exist");
    assert!(user_roles.contains(roles::MINTER));
}

#[test]
fn handover_mint_authority_wrong_token_id() {
    let harness = ItsTestHarness::new();

    let mint_authority = harness.get_new_wallet();
    let token_mint = harness.create_spl_token_mint(mint_authority, 9, None);
    let deployer = harness.get_new_wallet();
    let salt = [11u8; 32];

    let _token_id =
        register_custom_token(&harness, deployer, token_mint, salt, Type::MintBurn, None);

    let wrong_token_id = [99u8; 32];
    let (ix, _) = make_handover_mint_authority_instruction(
        harness.payer,
        mint_authority,
        token_mint,
        spl_token_2022::ID,
        wrong_token_id,
    );

    harness.ctx.process_and_validate_instruction(
        &ix,
        &[Check::err(
            anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::AccountNotInitialized)
                .into(),
        )],
    );
}

#[test]
fn handover_mint_authority_not_mint_authority() {
    let harness = ItsTestHarness::new();

    let mint_authority = harness.get_new_wallet();
    let token_mint = harness.create_spl_token_mint(mint_authority, 9, None);
    let deployer = harness.get_new_wallet();
    let salt = [12u8; 32];

    let token_id =
        register_custom_token(&harness, deployer, token_mint, salt, Type::MintBurn, None);

    let fake_authority = harness.get_new_wallet();
    let (ix, _) = make_handover_mint_authority_instruction(
        harness.payer,
        fake_authority,
        token_mint,
        spl_token_2022::ID,
        token_id,
    );

    harness.ctx.process_and_validate_instruction(
        &ix,
        &[Check::err(
            anchor_lang::error::Error::from(
                anchor_lang::error::ErrorCode::ConstraintMintMintAuthority,
            )
            .into(),
        )],
    );
}

#[test]
fn handover_mint_authority_non_mint_burn_token() {
    let harness = ItsTestHarness::new();

    let mint_authority = harness.get_new_wallet();
    let token_mint = harness.create_spl_token_mint(mint_authority, 9, None);
    let deployer = harness.get_new_wallet();
    let salt = [13u8; 32];

    let token_id =
        register_custom_token(&harness, deployer, token_mint, salt, Type::LockUnlock, None);

    let (ix, _) = make_handover_mint_authority_instruction(
        harness.payer,
        mint_authority,
        token_mint,
        spl_token_2022::ID,
        token_id,
    );

    harness.ctx.process_and_validate_instruction(
        &ix,
        &[Check::err(ItsError::InvalidTokenManagerType.into())],
    );
}
