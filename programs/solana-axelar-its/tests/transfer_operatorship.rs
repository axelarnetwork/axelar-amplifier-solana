#![cfg(test)]
#![allow(clippy::too_many_lines)]

use anchor_lang::AccountDeserialize;
use mollusk_svm::program::keyed_account_for_system_program;
use mollusk_test_utils::setup_mollusk;
use solana_axelar_its::state::{Roles, UserRoles};
use solana_axelar_its_test_fixtures::init_its_service;
use {
    anchor_lang::{solana_program::instruction::Instruction, InstructionData, ToAccountMetas},
    solana_sdk::{account::Account, pubkey::Pubkey},
};

#[test]
fn test_transfer_operatorship_success() {
    let program_id = solana_axelar_its::id();
    let mollusk = setup_mollusk(&program_id, "solana_axelar_its");

    let upgrade_authority = Pubkey::new_unique();
    let payer = upgrade_authority;
    let payer_account = Account::new(1_000_000_000, 0, &solana_sdk::system_program::ID);

    let current_operator = Pubkey::new_unique();
    let current_operator_account = Account::new(1_000_000_000, 0, &solana_sdk::system_program::ID);

    let new_operator = Pubkey::new_unique();
    let new_operator_account = Account::new(1_000_000_000, 0, &solana_sdk::system_program::ID);

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
        (
            new_operator_roles_pda,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
    ];

    let result = mollusk.process_instruction(&ix, &accounts);

    assert!(result.program_result.is_ok());

    let updated_current_roles_account = result
        .get_account(&current_operator_roles_pda)
        .expect("Current operator roles account should exist");

    let updated_current_roles =
        UserRoles::try_deserialize(&mut updated_current_roles_account.data.as_slice())
            .expect("Failed to deserialize updated current operator roles");

    // Current operator should no longer have OPERATOR role
    assert!(
        !updated_current_roles.roles.contains(Roles::OPERATOR),
        "Current operator should no longer have OPERATOR role"
    );

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
