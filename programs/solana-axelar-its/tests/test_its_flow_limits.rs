#![cfg(test)]

use mollusk_harness::{ItsTestHarness, TestHarness};
use mollusk_svm::result::Check;
use solana_axelar_its::instructions::make_set_flow_limit_instruction;
use solana_axelar_its::{ItsError, Roles, RolesError, TokenManager, UserRoles};

#[test]
fn test_set_flow_limit() {
    let harness = ItsTestHarness::new();

    let token_id = harness.ensure_test_interchain_token();

    // Verify initial flow limit is None
    let token_manager_pda = TokenManager::find_pda(token_id, harness.its_root).0;
    let token_manager: TokenManager = harness
        .get_account_as(&token_manager_pda)
        .expect("token manager should exist");
    assert_eq!(token_manager.flow_slot.flow_limit, None);

    // Set flow limit
    let flow_limit = Some(1_000_000_000u64);

    let (ix, _) =
        make_set_flow_limit_instruction(harness.payer, harness.operator, token_id, flow_limit);

    harness
        .ctx
        .process_and_validate_instruction(&ix, &[Check::success()]);

    // Verify flow limit was set
    let updated_token_manager: TokenManager = harness
        .get_account_as(&token_manager_pda)
        .expect("token manager should exist");
    assert_eq!(updated_token_manager.flow_slot.flow_limit, flow_limit);
}

#[test]
fn test_set_flow_limit_remove() {
    let harness = ItsTestHarness::new();

    let token_id = harness.ensure_test_interchain_token();

    // First set a flow limit
    let (set_ix, _) = make_set_flow_limit_instruction(
        harness.payer,
        harness.operator,
        token_id,
        Some(1_000_000u64),
    );

    harness
        .ctx
        .process_and_validate_instruction(&set_ix, &[Check::success()]);

    // Now remove flow limit (set to None)
    let (remove_ix, _) =
        make_set_flow_limit_instruction(harness.payer, harness.operator, token_id, None);

    harness
        .ctx
        .process_and_validate_instruction(&remove_ix, &[Check::success()]);

    // Verify flow limit is None
    let token_manager_pda = TokenManager::find_pda(token_id, harness.its_root).0;
    let token_manager: TokenManager = harness
        .get_account_as(&token_manager_pda)
        .expect("token manager should exist");
    assert_eq!(token_manager.flow_slot.flow_limit, None);
}

#[test]
fn test_set_flow_limit_same_value_fails() {
    let harness = ItsTestHarness::new();

    let token_id = harness.ensure_test_interchain_token();

    // Try to set flow limit to None (same as current)
    let (ix, _) = make_set_flow_limit_instruction(
        harness.payer,
        harness.operator,
        token_id,
        None, // Already None
    );

    harness
        .ctx
        .process_and_validate_instruction(&ix, &[Check::err(ItsError::InvalidArgument.into())]);
}

#[test]
fn test_set_flow_limit_unauthorized_operator_fails() {
    let harness = ItsTestHarness::new();

    let token_id = harness.ensure_test_interchain_token();

    let unauthorized = harness.get_new_wallet();

    let (ix, _) =
        make_set_flow_limit_instruction(harness.payer, unauthorized, token_id, Some(1_000_000u64));

    // Should fail because its_roles_pda derived from unauthorized doesn't exist
    harness.ctx.process_and_validate_instruction(
        &ix,
        &[Check::err(
            anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::AccountNotInitialized)
                .into(),
        )],
    );
}

#[test]
fn test_set_flow_limit_without_operator_role_fails() {
    let mut harness = ItsTestHarness::new();

    let token_id = harness.ensure_test_interchain_token();

    // Remove operator role
    let its_roles_pda = UserRoles::find_pda(&harness.its_root, &harness.operator).0;
    harness.update_account::<UserRoles, _>(&its_roles_pda, |ur| {
        ur.roles.remove(Roles::OPERATOR);
    });

    let (ix, _) = make_set_flow_limit_instruction(
        harness.payer,
        harness.operator,
        token_id,
        Some(1_000_000u64),
    );

    harness.ctx.process_and_validate_instruction(
        &ix,
        &[Check::err(RolesError::MissingOperatorRole.into())],
    );
}

#[test]
fn test_set_flow_limit_nonexistent_token_fails() {
    let harness = ItsTestHarness::new();

    // Don't deploy any token, try to set flow limit
    let fake_token_id = [99u8; 32];

    let (ix, _) = make_set_flow_limit_instruction(
        harness.payer,
        harness.operator,
        fake_token_id,
        Some(1_000_000u64),
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
fn test_set_flow_limit_update_existing() {
    let harness = ItsTestHarness::new();

    let token_id = harness.ensure_test_interchain_token();
    let token_manager_pda = TokenManager::find_pda(token_id, harness.its_root).0;

    // Set initial flow limit
    let (ix1, _) = make_set_flow_limit_instruction(
        harness.payer,
        harness.operator,
        token_id,
        Some(1_000_000u64),
    );
    harness
        .ctx
        .process_and_validate_instruction(&ix1, &[Check::success()]);

    // Update to new value
    let (ix2, _) = make_set_flow_limit_instruction(
        harness.payer,
        harness.operator,
        token_id,
        Some(2_000_000u64),
    );
    harness
        .ctx
        .process_and_validate_instruction(&ix2, &[Check::success()]);

    // Verify updated
    let token_manager: TokenManager = harness
        .get_account_as(&token_manager_pda)
        .expect("token manager should exist");
    assert_eq!(token_manager.flow_slot.flow_limit, Some(2_000_000u64));
}

#[test]
fn test_set_flow_limit_multiple_tokens() {
    let harness = ItsTestHarness::new();

    // Deploy two tokens with different salts
    let token_id_1 = harness.ensure_deploy_local_interchain_token(
        harness.operator,
        [1u8; 32],
        "Token One".to_owned(),
        "ONE".to_owned(),
        9,
        1_000_000,
        None,
    );

    let token_id_2 = harness.ensure_deploy_local_interchain_token(
        harness.operator,
        [2u8; 32],
        "Token Two".to_owned(),
        "TWO".to_owned(),
        9,
        1_000_000,
        None,
    );

    // Set different flow limits for each
    let (ix1, _) = make_set_flow_limit_instruction(
        harness.payer,
        harness.operator,
        token_id_1,
        Some(100_000u64),
    );
    harness
        .ctx
        .process_and_validate_instruction(&ix1, &[Check::success()]);

    let (ix2, _) = make_set_flow_limit_instruction(
        harness.payer,
        harness.operator,
        token_id_2,
        Some(500_000u64),
    );
    harness
        .ctx
        .process_and_validate_instruction(&ix2, &[Check::success()]);

    // Verify each token has correct flow limit
    let tm1_pda = TokenManager::find_pda(token_id_1, harness.its_root).0;
    let tm1: TokenManager = harness.get_account_as(&tm1_pda).unwrap();
    assert_eq!(tm1.flow_slot.flow_limit, Some(100_000u64));

    let tm2_pda = TokenManager::find_pda(token_id_2, harness.its_root).0;
    let tm2: TokenManager = harness.get_account_as(&tm2_pda).unwrap();
    assert_eq!(tm2.flow_slot.flow_limit, Some(500_000u64));
}
