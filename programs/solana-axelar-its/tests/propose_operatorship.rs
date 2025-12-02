#![cfg(test)]
#![allow(clippy::too_many_lines)]

use anchor_lang::{prelude::borsh, AccountDeserialize, Discriminator};
use mollusk_svm::result::Check;
use mollusk_test_utils::setup_mollusk;
use solana_axelar_its::{
    state::{RoleProposal, roles, UserRoles},
    ItsError,
};
use solana_axelar_its_test_fixtures::{
    init_its_service, new_default_account, new_test_account, propose_operatorship_helper,
    ProposeOperatorshipContext,
};
use solana_sdk::{account::Account, pubkey::Pubkey};

#[test]
fn test_propose_operatorship() {
    let program_id = solana_axelar_its::id();
    let mollusk = setup_mollusk(&program_id, "solana_axelar_its");

    let upgrade_authority = Pubkey::new_unique();
    let payer = upgrade_authority;
    let payer_account = new_default_account();

    let (current_operator, current_operator_account) = new_test_account();

    let proposed_operator = Pubkey::new_unique();
    let proposed_operator_account = new_default_account();

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
        (proposed_operator, proposed_operator_account.clone()),
    );

    let checks = vec![Check::success()];
    let (result, _) = propose_operatorship_helper(ctx, checks);

    assert!(result.program_result.is_ok());

    let (proposal_pda, _bump) = RoleProposal::find_pda(
        &its_root_pda,
        &current_operator,
        &proposed_operator,
        &program_id,
    );

    let proposal_account = result
        .get_account(&proposal_pda)
        .expect("Proposal account should exist");

    let proposal_data = RoleProposal::try_deserialize(&mut proposal_account.data.as_slice())
        .expect("Failed to deserialize proposal account");

    assert_eq!(proposal_data.roles, roles::OPERATOR);
}

#[test]
fn test_propose_malicious_operatorship_failure() {
    let program_id = solana_axelar_its::id();
    let mollusk = setup_mollusk(&program_id, "solana_axelar_its");

    let upgrade_authority = Pubkey::new_unique();
    let payer = upgrade_authority;
    let payer_account = new_default_account();

    let (current_operator, current_operator_account) = new_test_account();

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

    let attacker = Pubkey::new_unique();
    let malicious_proposed_operator = Pubkey::new_unique();
    let malicious_proposed_operator_account = new_default_account();

    let ctx = ProposeOperatorshipContext::new(
        mollusk,
        (payer, payer_account.clone()),
        (attacker, current_operator_account.clone()),
        (
            current_operator_roles_pda,
            current_operator_roles_account.clone(),
        ),
        (its_root_pda, its_root_account.clone()),
        (
            malicious_proposed_operator,
            malicious_proposed_operator_account.clone(),
        ),
    );

    let checks = vec![Check::err(
        anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::ConstraintSeeds).into(),
    )];

    let (result, _) = propose_operatorship_helper(ctx, checks);
    assert!(result.program_result.is_err());
}

#[test]
fn test_propose_self_failure() {
    let program_id = solana_axelar_its::id();
    let mollusk = setup_mollusk(&program_id, "solana_axelar_its");

    let upgrade_authority = Pubkey::new_unique();
    let payer = upgrade_authority;
    let payer_account = new_default_account();

    let (current_operator, current_operator_account) = new_test_account();

    let proposed_operator = current_operator;
    let proposed_operator_account = current_operator_account.clone();

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
        (proposed_operator, proposed_operator_account.clone()),
    );

    let checks = vec![Check::err(
        anchor_lang::error::Error::from(ItsError::InvalidArgument).into(),
    )];

    let (result, _) = propose_operatorship_helper(ctx, checks);
    assert!(result.program_result.is_err());
}

#[test]
fn test_propose_operatorship_missing_operator_role_failure() {
    let program_id = solana_axelar_its::id();
    let mollusk = setup_mollusk(&program_id, "solana_axelar_its");

    let upgrade_authority = Pubkey::new_unique();
    let payer = upgrade_authority;
    let payer_account = new_default_account();

    let non_operator = Pubkey::new_unique();
    let non_operator_account = new_default_account();

    let proposed_operator = Pubkey::new_unique();
    let proposed_operator_account = new_default_account();

    let chain_name = "solana".to_owned();
    let its_hub_address = "0x123456789abcdef".to_owned();

    // Initialize ITS service with a proper operator first
    let (current_operator, current_operator_account) = new_test_account();

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

    let (non_operator_roles_pda, bump) = UserRoles::find_pda(&its_root_pda, &non_operator);

    // Create UserRoles data with missing operator role
    let user_roles_data = UserRoles {
        roles: roles::MINTER,
        bump,
    };

    let mut user_roles_serialized = Vec::new();
    user_roles_serialized.extend_from_slice(UserRoles::DISCRIMINATOR);
    user_roles_serialized.extend_from_slice(&borsh::to_vec(&user_roles_data).unwrap());

    let non_operator_roles_account = Account {
        lamports: 1_000_000,
        data: user_roles_serialized,
        owner: program_id,
        executable: false,
        rent_epoch: 0,
    };

    let ctx = ProposeOperatorshipContext::new(
        mollusk,
        (payer, payer_account.clone()),
        (non_operator, non_operator_account.clone()),
        (non_operator_roles_pda, non_operator_roles_account),
        (its_root_pda, its_root_account.clone()),
        (proposed_operator, proposed_operator_account.clone()),
    );

    let checks = vec![Check::err(
        anchor_lang::error::Error::from(solana_axelar_its::state::RolesError::MissingOperatorRole)
            .into(),
    )];

    let (result, _) = propose_operatorship_helper(ctx, checks);
    assert!(result.program_result.is_err());
}
