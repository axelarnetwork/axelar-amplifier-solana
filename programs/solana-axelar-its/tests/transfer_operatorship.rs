#![cfg(test)]
#![allow(clippy::too_many_lines)]

use anchor_lang::{AccountDeserialize, AnchorSerialize, Discriminator};
use mollusk_svm::{program::keyed_account_for_system_program, result::Check};
use mollusk_test_utils::setup_mollusk;
use solana_axelar_its::state::{Roles, RolesError, UserRoles};
use solana_axelar_its_test_fixtures::{
    init_its_service, new_default_account, new_empty_account, new_test_account,
};
use {
    anchor_lang::{solana_program::instruction::Instruction, InstructionData, ToAccountMetas},
    solana_sdk::pubkey::Pubkey,
};

#[test]
fn test_transfer_operatorship_success() {
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
    assert!(current_roles_data.roles.contains(Roles::OPERATOR));

    let (new_operator_roles_pda, new_operator_roles_pda_bump) = Pubkey::find_program_address(
        &UserRoles::pda_seeds(&its_root_pda, &new_operator)[..],
        &program_id,
    );

    let ix = Instruction {
        program_id,
        accounts: solana_axelar_its::accounts::TransferOperatorship {
            system_program: solana_sdk::system_program::ID,
            payer,
            origin_user_account: current_operator,
            origin_roles_account: current_operator_roles_pda,
            resource_account: its_root_pda,
            destination_user_account: new_operator,
            destination_roles_account: new_operator_roles_pda,
        }
        .to_account_metas(None),
        data: solana_axelar_its::instruction::TransferOperatorship {}.data(),
    };

    let accounts = vec![
        keyed_account_for_system_program(),
        (payer, payer_account.clone()),
        (current_operator, current_operator_account.clone()),
        (
            current_operator_roles_pda,
            current_operator_roles_account.clone(),
        ),
        (its_root_pda, its_root_account.clone()),
        (new_operator, new_operator_account.clone()),
        (new_operator_roles_pda, new_empty_account()),
    ];

    let result = mollusk.process_instruction(&ix, &accounts);

    assert!(result.program_result.is_ok());

    let updated_current_roles_account = result
        .get_account(&current_operator_roles_pda)
        .expect("Current operator roles account should exist");

    assert!(updated_current_roles_account.data.is_empty()); // Account is closed

    let new_operator_roles_account = result
        .get_account(&new_operator_roles_pda)
        .expect("New operator roles account should exist");

    let new_operator_roles =
        UserRoles::try_deserialize(&mut new_operator_roles_account.data.as_slice())
            .expect("Failed to deserialize new operator roles");

    assert_eq!(new_operator_roles.bump, new_operator_roles_pda_bump);

    assert!(
        new_operator_roles.roles.contains(Roles::OPERATOR),
        "New operator should have OPERATOR role"
    );
}

#[test]
fn test_reject_transfer_operatorship_with_invalid_authority() {
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
    assert!(current_roles_data.roles.contains(Roles::OPERATOR));

    let (new_operator_roles_pda, _) = Pubkey::find_program_address(
        &UserRoles::pda_seeds(&its_root_pda, &new_operator)[..],
        &program_id,
    );

    let invalid_current_operator = Pubkey::new_unique();

    let ix = Instruction {
        program_id,
        accounts: solana_axelar_its::accounts::TransferOperatorship {
            system_program: solana_sdk::system_program::ID,
            payer,
            origin_user_account: invalid_current_operator,
            origin_roles_account: current_operator_roles_pda,
            resource_account: its_root_pda,
            destination_user_account: new_operator,
            destination_roles_account: new_operator_roles_pda,
        }
        .to_account_metas(None),
        data: solana_axelar_its::instruction::TransferOperatorship {}.data(),
    };

    let accounts = vec![
        keyed_account_for_system_program(),
        (payer, payer_account.clone()),
        (invalid_current_operator, current_operator_account.clone()),
        (
            current_operator_roles_pda,
            current_operator_roles_account.clone(),
        ),
        (its_root_pda, its_root_account.clone()),
        (new_operator, new_operator_account.clone()),
        (new_operator_roles_pda, new_empty_account()),
    ];

    let checks = vec![Check::err(
        anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::ConstraintSeeds).into(),
    )];

    mollusk.process_and_validate_instruction(&ix, &accounts, &checks);
}

#[test]
fn test_reject_transfer_operatorship_without_operator_role() {
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
    assert!(current_roles_data.roles.contains(Roles::OPERATOR));

    let (new_operator_roles_pda, _) = Pubkey::find_program_address(
        &UserRoles::pda_seeds(&its_root_pda, &new_operator)[..],
        &program_id,
    );

    let ix = Instruction {
        program_id,
        accounts: solana_axelar_its::accounts::TransferOperatorship {
            system_program: solana_sdk::system_program::ID,
            payer,
            origin_user_account: current_operator,
            origin_roles_account: current_operator_roles_pda,
            resource_account: its_root_pda,
            destination_user_account: new_operator,
            destination_roles_account: new_operator_roles_pda,
        }
        .to_account_metas(None),
        data: solana_axelar_its::instruction::TransferOperatorship {}.data(),
    };

    let mut current_operator_roles_account_clone = current_operator_roles_account.clone();

    let mut current_operator_roles =
        UserRoles::try_deserialize(&mut current_operator_roles_account_clone.data.as_ref())
            .expect("Failed to deserialize roles");

    current_operator_roles.roles = Roles::empty();

    let mut new_data = Vec::new();
    new_data.extend_from_slice(UserRoles::DISCRIMINATOR);
    current_operator_roles
        .serialize(&mut new_data)
        .expect("Failed to serialize");

    current_operator_roles_account_clone.data = new_data;

    let accounts = vec![
        keyed_account_for_system_program(),
        (payer, payer_account.clone()),
        (current_operator, current_operator_account.clone()),
        (
            current_operator_roles_pda,
            current_operator_roles_account_clone,
        ),
        (its_root_pda, its_root_account.clone()),
        (new_operator, new_operator_account.clone()),
        (new_operator_roles_pda, new_empty_account()),
    ];

    let checks = vec![Check::err(
        anchor_lang::error::Error::from(RolesError::MissingOperatorRole).into(),
    )];

    mollusk.process_and_validate_instruction(&ix, &accounts, &checks);
}
