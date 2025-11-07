#![cfg(test)]
#![allow(clippy::too_many_lines)]

use anchor_lang::{prelude::borsh, AccountDeserialize, Discriminator};
use mollusk_svm::{program::keyed_account_for_system_program, result::Check};
use mollusk_test_utils::setup_mollusk;
use solana_axelar_its::state::{RoleProposal, Roles, UserRoles};
use solana_axelar_its_test_fixtures::init_its_service;
use {
    anchor_lang::{solana_program::instruction::Instruction, InstructionData, ToAccountMetas},
    solana_sdk::{account::Account, pubkey::Pubkey},
};

#[test]
fn test_accept_operatorship() {
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

    let (proposal_pda, _bump) =
        RoleProposal::find_pda(&its_root_pda, &current_operator, &new_operator, &program_id);

    let propose_ix = Instruction {
        program_id,
        accounts: solana_axelar_its::accounts::ProposeOperatorship {
            system_program: solana_sdk::system_program::ID,
            payer,
            origin_user_account: current_operator,
            origin_roles_account: current_operator_roles_pda,
            resource_account: its_root_pda,
            destination_user_account: new_operator,
            proposal_account: proposal_pda,
        }
        .to_account_metas(None),
        data: solana_axelar_its::instruction::ProposeOperatorship {}.data(),
    };

    let propose_accounts = vec![
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
            proposal_pda,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
    ];

    let propose_result = mollusk.process_instruction(&propose_ix, &propose_accounts);
    assert!(propose_result.program_result.is_ok());

    let proposal_account_after_propose = propose_result
        .get_account(&proposal_pda)
        .expect("Proposal account should exist after propose");

    // Now create the destination roles PDA
    let (new_operator_roles_pda, _bump) = Pubkey::find_program_address(
        &UserRoles::pda_seeds(&its_root_pda, &new_operator)[..],
        &program_id,
    );

    let accept_ix = Instruction {
        program_id,
        accounts: solana_axelar_its::accounts::AcceptOperatorship {
            system_program: solana_sdk::system_program::ID,
            payer,
            destination_user_account: new_operator,
            destination_roles_account: new_operator_roles_pda,
            resource_account: its_root_pda,
            origin_user_account: current_operator,
            origin_roles_account: current_operator_roles_pda,
            proposal_account: proposal_pda,
        }
        .to_account_metas(None),
        data: solana_axelar_its::instruction::AcceptOperatorship {}.data(),
    };

    let accept_accounts = vec![
        keyed_account_for_system_program(),
        (payer, payer_account.clone()),
        (new_operator, new_operator_account.clone()),
        (
            new_operator_roles_pda,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
        (its_root_pda, its_root_account.clone()),
        (current_operator, current_operator_account.clone()),
        (
            current_operator_roles_pda,
            current_operator_roles_account.clone(),
        ),
        (proposal_pda, proposal_account_after_propose.clone()),
    ];

    let accept_result = mollusk.process_instruction(&accept_ix, &accept_accounts);
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

    assert!(new_operator_roles.roles.contains(Roles::OPERATOR));

    // Check that proposal was closed
    let proposal_account = accept_result.get_account(&proposal_pda).unwrap();
    assert!(proposal_account.data.is_empty());
}

#[test]
fn test_accept_invalid_operatorship() {
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

    let fake_origin = Pubkey::new_unique();
    let (wrong_proposal_pda, fake_bump) =
        RoleProposal::find_pda(&its_root_pda, &fake_origin, &new_operator, &program_id);

    let role_proposal_data = RoleProposal {
        roles: Roles::OPERATOR,
        bump: fake_bump,
    };

    let mut proposal_account_data = Vec::new();
    proposal_account_data.extend_from_slice(&RoleProposal::DISCRIMINATOR);
    proposal_account_data.extend_from_slice(&borsh::to_vec(&role_proposal_data).unwrap());

    let wrong_proposal_account = Account {
        lamports: 1_000_000,
        data: proposal_account_data,
        owner: program_id,
        executable: false,
        rent_epoch: 0,
    };

    // Now create the destination roles PDA
    let (new_operator_roles_pda, _bump) = Pubkey::find_program_address(
        &UserRoles::pda_seeds(&its_root_pda, &new_operator)[..],
        &program_id,
    );

    let accept_ix = Instruction {
        program_id,
        accounts: solana_axelar_its::accounts::AcceptOperatorship {
            system_program: solana_sdk::system_program::ID,
            payer,
            destination_user_account: new_operator,
            destination_roles_account: new_operator_roles_pda,
            resource_account: its_root_pda,
            origin_user_account: current_operator,
            origin_roles_account: current_operator_roles_pda,
            proposal_account: wrong_proposal_pda,
        }
        .to_account_metas(None),
        data: solana_axelar_its::instruction::AcceptOperatorship {}.data(),
    };

    let accept_accounts = vec![
        keyed_account_for_system_program(),
        (payer, payer_account.clone()),
        (new_operator, new_operator_account.clone()),
        (
            new_operator_roles_pda,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
        (its_root_pda, its_root_account.clone()),
        (current_operator, current_operator_account.clone()),
        (
            current_operator_roles_pda,
            current_operator_roles_account.clone(),
        ),
        (wrong_proposal_pda, wrong_proposal_account.clone()),
    ];

    let checks = vec![Check::err(
        anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::ConstraintSeeds).into(),
    )];

    mollusk.process_and_validate_instruction(&accept_ix, &accept_accounts, &checks);
}
