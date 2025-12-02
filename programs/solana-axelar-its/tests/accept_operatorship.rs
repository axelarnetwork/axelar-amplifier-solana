#![cfg(test)]
#![allow(clippy::too_many_lines)]

use anchor_lang::{prelude::borsh, AccountDeserialize, Discriminator};
use mollusk_svm::result::Check;
use mollusk_test_utils::setup_mollusk;
use solana_axelar_its::state::{RoleProposal, roles, UserRoles};
use solana_axelar_its_test_fixtures::{
    accept_operatorship_helper, init_its_service, new_default_account, new_empty_account,
    new_test_account, propose_operatorship_helper, AcceptOperatorshipContext,
    ProposeOperatorshipContext,
};
use solana_sdk::{account::Account, pubkey::Pubkey};

#[test]
fn test_accept_operatorship() {
    let program_id = solana_axelar_its::id();
    let mollusk = setup_mollusk(&program_id, "solana_axelar_its");

    let upgrade_authority = Pubkey::new_unique();
    let payer = upgrade_authority;
    let payer_account = new_default_account();

    let (current_operator, current_operator_account) = new_test_account();

    let (new_operator, new_operator_account) = new_test_account();

    let chain_name = "solana".to_owned();
    let its_hub_address = "0x123456789abcdef".to_owned();

    let (
        its_root_pda,
        its_root_account,
        current_operator_roles_pda,
        current_operator_roles_account,
        _,
        _,
    ) = init_its_service(
        &mollusk,
        payer,
        &payer_account,
        upgrade_authority,
        current_operator,
        &current_operator_account,
        chain_name.clone(),
        its_hub_address.clone(),
    );

    let current_roles_data =
        UserRoles::try_deserialize(&mut current_operator_roles_account.data.as_slice())
            .expect("Failed to deserialize current operator roles");
    assert!(current_roles_data.contains(roles::OPERATOR));

    let ctx = ProposeOperatorshipContext::new(
        mollusk,
        (payer, payer_account.clone()),
        (current_operator, current_operator_account.clone()),
        (
            current_operator_roles_pda,
            current_operator_roles_account.clone(),
        ),
        (its_root_pda, its_root_account.clone()),
        (new_operator, new_operator_account.clone()),
    );

    let checks = vec![Check::success()];
    let (propose_result, mollusk) = propose_operatorship_helper(ctx, checks);

    let (proposal_pda, _bump) =
        RoleProposal::find_pda(&its_root_pda, &current_operator, &new_operator, &program_id);
    assert!(propose_result.program_result.is_ok());

    let proposal_account_after_propose = propose_result
        .get_account(&proposal_pda)
        .expect("Proposal account should exist after propose");

    // Now create the destination roles PDA
    let (new_operator_roles_pda, _bump) = UserRoles::find_pda(&its_root_pda, &new_operator);

    let accept_ctx = AcceptOperatorshipContext::new(
        mollusk,
        (payer, payer_account.clone()),
        (new_operator, new_operator_account.clone()),
        (its_root_pda, its_root_account.clone()),
        (current_operator, current_operator_account.clone()),
        (
            current_operator_roles_pda,
            current_operator_roles_account.clone(),
        ),
        (proposal_pda, proposal_account_after_propose.clone()),
    );

    let checks = vec![Check::success()];
    let (accept_result, _) = accept_operatorship_helper(accept_ctx, checks);
    assert!(accept_result.program_result.is_ok());

    // Verify the operatorship transfer
    let updated_current_roles_account = accept_result
        .get_account(&current_operator_roles_pda)
        .expect("Current operator roles account should exist");

    assert!(updated_current_roles_account.data.is_empty()); // Account should be closed

    let new_operator_roles_account = accept_result
        .get_account(&new_operator_roles_pda)
        .expect("New operator roles account should exist");

    let new_operator_roles =
        UserRoles::try_deserialize(&mut new_operator_roles_account.data.as_slice())
            .expect("Failed to deserialize new operator roles");

    assert!(new_operator_roles.contains(roles::OPERATOR));

    // Check that proposal was closed
    let proposal_account = accept_result.get_account(&proposal_pda).unwrap();
    assert!(proposal_account.data.is_empty());
}

#[test]
fn test_reject_invalid_operatorship() {
    let program_id = solana_axelar_its::id();
    let mollusk = setup_mollusk(&program_id, "solana_axelar_its");

    let upgrade_authority = Pubkey::new_unique();
    let payer = upgrade_authority;
    let payer_account = new_default_account();

    let (current_operator, current_operator_account) = new_test_account();
    let (new_operator, new_operator_account) = new_test_account();

    let chain_name = "solana".to_owned();
    let its_hub_address = "0x123456789abcdef".to_owned();

    let (
        its_root_pda,
        its_root_account,
        current_operator_roles_pda,
        current_operator_roles_account,
        _program_data,
        _program_data_account,
    ) = init_its_service(
        &mollusk,
        payer,
        &payer_account,
        upgrade_authority,
        current_operator,
        &current_operator_account,
        chain_name.clone(),
        its_hub_address.clone(),
    );

    let current_roles_data =
        UserRoles::try_deserialize(&mut current_operator_roles_account.data.as_slice())
            .expect("Failed to deserialize current operator roles");
    assert!(current_roles_data.contains(roles::OPERATOR));

    let fake_origin = Pubkey::new_unique();
    let (wrong_proposal_pda, fake_bump) =
        RoleProposal::find_pda(&its_root_pda, &fake_origin, &new_operator, &program_id);

    let role_proposal_data = RoleProposal {
        roles: roles::OPERATOR,
        bump: fake_bump,
    };

    let mut proposal_account_data = Vec::new();
    proposal_account_data.extend_from_slice(RoleProposal::DISCRIMINATOR);
    proposal_account_data.extend_from_slice(&borsh::to_vec(&role_proposal_data).unwrap());

    let wrong_proposal_account = Account {
        lamports: 1_000_000,
        data: proposal_account_data,
        owner: program_id,
        executable: false,
        rent_epoch: 0,
    };

    // Now create the destination roles PDA
    let (new_operator_roles_pda, _bump) = UserRoles::find_pda(&its_root_pda, &new_operator);

    let accept_ctx = AcceptOperatorshipContext::with_custom_destination_roles_account(
        mollusk,
        (payer, payer_account.clone()),
        (new_operator, new_operator_account.clone()),
        (new_operator_roles_pda, new_empty_account()),
        (its_root_pda, its_root_account.clone()),
        (current_operator, current_operator_account.clone()),
        (
            current_operator_roles_pda,
            current_operator_roles_account.clone(),
        ),
        (wrong_proposal_pda, wrong_proposal_account.clone()),
    );

    let checks = vec![Check::err(
        anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::ConstraintSeeds).into(),
    )];

    let (result, _) = accept_operatorship_helper(accept_ctx, checks);
    assert!(result.program_result.is_err());
}
