#![cfg(test)]
#![allow(clippy::indexing_slicing)]

use anchor_spl::token_2022::spl_token_2022;
use mollusk_harness::{ItsTestHarness, TestHarness};
use mollusk_svm::result::Check;
use solana_axelar_its::{
    instructions::{
        make_add_token_manager_flow_limiter_instruction, make_register_custom_token_instruction,
        make_remove_token_manager_flow_limiter_instruction,
        make_set_token_manager_flow_limit_instruction,
        make_transfer_token_manager_operatorship_instruction,
    },
    state::{roles, token_manager::Type, InterchainTokenService, TokenManager, UserRoles},
    utils::{interchain_token_id_internal, linked_token_deployer_salt},
    ItsError,
};

/// Register a custom token with an operator, return (token_id, operator).
fn setup_custom_token_with_operator(
    harness: &ItsTestHarness,
) -> ([u8; 32], solana_sdk::pubkey::Pubkey) {
    let mint_authority = harness.get_new_wallet();
    let token_mint = harness.create_spl_token_mint(mint_authority, 9, None);
    let deployer = harness.get_new_wallet();
    let op = harness.get_new_wallet();
    let salt = [42u8; 32];

    let (ix, _) = make_register_custom_token_instruction(
        harness.payer,
        deployer,
        token_mint,
        spl_token_2022::ID,
        salt,
        Type::LockUnlock,
        Some(op),
    );

    harness
        .ctx
        .process_and_validate_instruction(&ix, &[Check::success()]);

    let deploy_salt = linked_token_deployer_salt(&deployer, &salt);
    let token_id = interchain_token_id_internal(&deploy_salt);
    (token_id, op)
}

// ── Add Flow Limiter ─────────────────────────────────────────────────

#[test]
fn add_flow_limiter_success() {
    let harness = ItsTestHarness::new();
    let (token_id, operator) = setup_custom_token_with_operator(&harness);

    let target = harness.get_new_wallet();

    let (ix, _) =
        make_add_token_manager_flow_limiter_instruction(harness.payer, operator, target, token_id);

    harness
        .ctx
        .process_and_validate_instruction(&ix, &[Check::success()]);

    // Verify target has FLOW_LIMITER role
    let its_root_pda = InterchainTokenService::find_pda().0;
    let token_manager_pda = TokenManager::find_pda(token_id, its_root_pda).0;
    let target_roles_pda = UserRoles::find_pda(&token_manager_pda, &target).0;
    let user_roles: UserRoles = harness
        .get_account_as(&target_roles_pda)
        .expect("target roles should exist");
    assert!(user_roles.has_flow_limiter_role());
}

#[test]
fn reject_add_flow_limiter_unauthorized() {
    let harness = ItsTestHarness::new();
    let (token_id, _operator) = setup_custom_token_with_operator(&harness);

    let unauthorized = harness.get_new_wallet();
    let target = harness.get_new_wallet();

    let (ix, _) = make_add_token_manager_flow_limiter_instruction(
        harness.payer,
        unauthorized,
        target,
        token_id,
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
fn reject_add_flow_limiter_without_operator_role() {
    let mut harness = ItsTestHarness::new();
    let (token_id, operator) = setup_custom_token_with_operator(&harness);

    // Strip operator role
    let its_root_pda = InterchainTokenService::find_pda().0;
    let token_manager_pda = TokenManager::find_pda(token_id, its_root_pda).0;
    let operator_roles_pda = UserRoles::find_pda(&token_manager_pda, &operator).0;
    harness.update_account_as::<UserRoles, _>(&operator_roles_pda, |r| {
        r.remove(roles::OPERATOR);
        r.insert(roles::MINTER); // keep some other role so account exists
    });

    let target = harness.get_new_wallet();

    let (ix, _) =
        make_add_token_manager_flow_limiter_instruction(harness.payer, operator, target, token_id);

    harness
        .ctx
        .process_and_validate_instruction(&ix, &[Check::err(ItsError::MissingOperatorRole.into())]);
}

// ── Remove Flow Limiter ──────────────────────────────────────────────

#[test]
fn remove_flow_limiter_success() {
    let harness = ItsTestHarness::new();
    let (token_id, operator) = setup_custom_token_with_operator(&harness);

    let target = harness.get_new_wallet();

    // First add the flow limiter role
    let (add_ix, _) =
        make_add_token_manager_flow_limiter_instruction(harness.payer, operator, target, token_id);
    harness
        .ctx
        .process_and_validate_instruction(&add_ix, &[Check::success()]);

    // Then remove it
    let (remove_ix, _) = make_remove_token_manager_flow_limiter_instruction(
        harness.payer,
        operator,
        target,
        token_id,
    );
    harness
        .ctx
        .process_and_validate_instruction(&remove_ix, &[Check::success()]);

    // Verify role is removed (account may be closed if no roles remain)
    let its_root_pda = InterchainTokenService::find_pda().0;
    let token_manager_pda = TokenManager::find_pda(token_id, its_root_pda).0;
    let target_roles_pda = UserRoles::find_pda(&token_manager_pda, &target).0;
    let user_roles: Option<UserRoles> = harness.get_account_as(&target_roles_pda);
    match user_roles {
        None => {} // closed — all good
        Some(r) => assert!(!r.has_flow_limiter_role()),
    }
}

#[test]
fn reject_remove_flow_limiter_unauthorized() {
    let harness = ItsTestHarness::new();
    let (token_id, operator) = setup_custom_token_with_operator(&harness);

    let target = harness.get_new_wallet();

    // Add flow limiter first
    let (add_ix, _) =
        make_add_token_manager_flow_limiter_instruction(harness.payer, operator, target, token_id);
    harness
        .ctx
        .process_and_validate_instruction(&add_ix, &[Check::success()]);

    // Try to remove with unauthorized account
    let unauthorized = harness.get_new_wallet();
    let (remove_ix, _) = make_remove_token_manager_flow_limiter_instruction(
        harness.payer,
        unauthorized,
        target,
        token_id,
    );

    harness.ctx.process_and_validate_instruction(
        &remove_ix,
        &[Check::err(
            anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::AccountNotInitialized)
                .into(),
        )],
    );
}

// ── Set Token Manager Flow Limit ─────────────────────────────────────

#[test]
fn set_token_manager_flow_limit_success() {
    let harness = ItsTestHarness::new();
    let (token_id, operator) = setup_custom_token_with_operator(&harness);

    // Operator also has FLOW_LIMITER from registration
    let (ix, _) = make_set_token_manager_flow_limit_instruction(
        harness.payer,
        operator,
        token_id,
        Some(1_000_000),
    );

    harness
        .ctx
        .process_and_validate_instruction(&ix, &[Check::success()]);

    // Verify flow limit is set
    let its_root_pda = InterchainTokenService::find_pda().0;
    let token_manager_pda = TokenManager::find_pda(token_id, its_root_pda).0;
    let tm: TokenManager = harness
        .get_account_as(&token_manager_pda)
        .expect("token manager should exist");
    assert_eq!(tm.flow_slot.flow_limit, Some(1_000_000));
}

#[test]
fn reject_set_token_manager_flow_limit_without_flow_limiter_role() {
    let harness = ItsTestHarness::new();
    let (token_id, _operator) = setup_custom_token_with_operator(&harness);

    let non_limiter = harness.get_new_wallet();

    let (ix, _) = make_set_token_manager_flow_limit_instruction(
        harness.payer,
        non_limiter,
        token_id,
        Some(1_000_000),
    );

    harness.ctx.process_and_validate_instruction(
        &ix,
        &[Check::err(
            anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::AccountNotInitialized)
                .into(),
        )],
    );
}

// ── Transfer Token Manager Operatorship ──────────────────────────────

#[test]
fn transfer_token_manager_operatorship_success() {
    let harness = ItsTestHarness::new();
    let (token_id, operator) = setup_custom_token_with_operator(&harness);

    let new_operator = harness.get_new_wallet();

    let (ix, _) = make_transfer_token_manager_operatorship_instruction(
        harness.payer,
        operator,
        new_operator,
        token_id,
    );

    harness
        .ctx
        .process_and_validate_instruction(&ix, &[Check::success()]);

    // Verify new operator has OPERATOR role
    let its_root_pda = InterchainTokenService::find_pda().0;
    let token_manager_pda = TokenManager::find_pda(token_id, its_root_pda).0;
    let new_roles_pda = UserRoles::find_pda(&token_manager_pda, &new_operator).0;
    let new_roles: UserRoles = harness
        .get_account_as(&new_roles_pda)
        .expect("new operator roles should exist");
    assert!(new_roles.has_operator_role());

    // Verify old operator lost OPERATOR role
    let old_roles_pda = UserRoles::find_pda(&token_manager_pda, &operator).0;
    let old_roles: Option<UserRoles> = harness.get_account_as(&old_roles_pda);
    match old_roles {
        None => {} // closed
        Some(r) => assert!(!r.has_operator_role()),
    }
}

#[test]
fn reject_transfer_token_manager_operatorship_unauthorized() {
    let harness = ItsTestHarness::new();
    let (token_id, _operator) = setup_custom_token_with_operator(&harness);

    let unauthorized = harness.get_new_wallet();
    let destination = harness.get_new_wallet();

    let (ix, _) = make_transfer_token_manager_operatorship_instruction(
        harness.payer,
        unauthorized,
        destination,
        token_id,
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
fn reject_transfer_token_manager_operatorship_same_sender_destination() {
    let harness = ItsTestHarness::new();
    let (token_id, operator) = setup_custom_token_with_operator(&harness);

    let (ix, _) = make_transfer_token_manager_operatorship_instruction(
        harness.payer,
        operator,
        operator, // same as sender
        token_id,
    );

    // The `dup` attribute on destination_roles_account fires before the
    // explicit key != key constraint when both resolve to the same PDA.
    harness.ctx.process_and_validate_instruction(
        &ix,
        &[Check::err(
            anchor_lang::error::Error::from(
                anchor_lang::error::ErrorCode::ConstraintDuplicateMutableAccount,
            )
            .into(),
        )],
    );
}

// ── Flow Limit Enforcement ───────────────────────────────────────────

#[test]
fn flow_limit_enforced_on_inbound_transfer() {
    let mut harness = ItsTestHarness::new();

    let token_id = harness.ensure_test_interchain_token();
    harness.ensure_trusted_chain("ethereum");

    // Set a flow limit via the ITS-level set_flow_limit
    let (set_limit_ix, _) = solana_axelar_its::instructions::make_set_flow_limit_instruction(
        harness.payer,
        harness.operator,
        token_id,
        Some(500),
    );
    harness
        .ctx
        .process_and_validate_instruction(&set_limit_ix, &[Check::success()]);

    // Transfer within limit — should succeed
    let receiver = harness.get_new_wallet();
    harness.execute_gmp_transfer(token_id, "ethereum", "eth_addr", receiver, 400, None);

    // Transfer exceeding limit — should fail
    let receiver2 = harness.get_new_wallet();
    harness.execute_gmp_transfer_with_authority(
        token_id,
        "ethereum",
        "eth_addr",
        receiver2,
        200, // 400 + 200 = 600 > 500 limit
        None,
        receiver2,
        &[Check::err(
            solana_axelar_its::ItsError::FlowLimitExceeded.into(),
        )],
    );
}

// ── GMP Deploy Interchain Token ──────────────────────────────────────

#[test]
fn execute_gmp_deploy_interchain_token() {
    use anchor_lang::prelude::AccountMeta;

    let mut harness = ItsTestHarness::new();
    harness.ensure_trusted_chain("ethereum");

    let token_id = [7u8; 32];

    let deploy_payload = solana_axelar_its::encoding::DeployInterchainToken {
        token_id,
        name: "Remote Token".to_owned(),
        symbol: "RTK".to_owned(),
        decimals: 6,
        minter: None,
    };

    let hub_message = solana_axelar_its::encoding::HubMessage::ReceiveFromHub {
        source_chain: "ethereum".to_owned(),
        message: solana_axelar_its::encoding::Message::DeployInterchainToken(deploy_payload),
    };

    // GMP deploy needs extra remaining accounts:
    // sysvar_instructions, mpl_token_metadata_program, mpl_token_metadata_account
    let (metadata_pda, _) = mpl_token_metadata::accounts::Metadata::find_pda(
        &solana_axelar_its::TokenManager::find_token_mint(token_id, harness.its_root).0,
    );

    let extra_accounts = vec![
        AccountMeta::new_readonly(solana_sdk::sysvar::instructions::id(), false),
        AccountMeta::new_readonly(mpl_token_metadata::programs::MPL_TOKEN_METADATA_ID, false),
        AccountMeta::new(metadata_pda, false),
    ];

    harness.execute_hub_message(token_id, "ethereum", hub_message, extra_accounts);

    // Verify token manager was created
    let token_manager_pda = TokenManager::find_pda(token_id, harness.its_root).0;
    let tm: TokenManager = harness
        .get_account_as(&token_manager_pda)
        .expect("token manager should exist");
    assert_eq!(tm.ty, Type::NativeInterchainToken);
}
