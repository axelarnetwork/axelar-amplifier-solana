#![cfg(test)]
#![allow(clippy::too_many_lines)]

use anchor_lang::AccountDeserialize;
use mollusk_svm::program::keyed_account_for_system_program;
use mollusk_test_utils::setup_mollusk;
use solana_axelar_its::state::{RoleProposal, Roles, UserRoles};
use solana_axelar_its_test_fixtures::init_its_service;
use {
    anchor_lang::{solana_program::instruction::Instruction, InstructionData, ToAccountMetas},
    solana_sdk::{account::Account, pubkey::Pubkey},
};

#[test]
fn test_propose_operatorship() {
    let program_id = solana_axelar_its::id();
    let mollusk = setup_mollusk(&program_id, "solana_axelar_its");

    let upgrade_authority = Pubkey::new_unique();
    let payer = upgrade_authority;
    let payer_account = Account::new(1_000_000_000, 0, &solana_sdk::system_program::ID);

    let current_operator = Pubkey::new_unique();
    let current_operator_account = Account::new(1_000_000_000, 0, &solana_sdk::system_program::ID);

    let proposed_operator = Pubkey::new_unique();
    let proposed_operator_account = Account::new(1_000_000_000, 0, &solana_sdk::system_program::ID);

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

    let (proposal_pda, _bump) = RoleProposal::find_pda(
        &its_root_pda,
        &current_operator,
        &proposed_operator,
        &program_id,
    );

    let ix = Instruction {
        program_id,
        accounts: solana_axelar_its::accounts::ProposeOperatorship {
            system_program: solana_sdk::system_program::ID,
            payer,
            origin_user_account: current_operator,
            origin_roles_account: current_operator_roles_pda,
            resource_account: its_root_pda,
            destination_user_account: proposed_operator,
            proposal_account: proposal_pda,
        }
        .to_account_metas(None),
        data: solana_axelar_its::instruction::ProposeOperatorship {}.data(),
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
        (proposed_operator, proposed_operator_account.clone()),
        (
            proposal_pda,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
    ];

    let result = mollusk.process_instruction(&ix, &accounts);

    assert!(result.program_result.is_ok());

    let proposal_account = result
        .get_account(&proposal_pda)
        .expect("Proposal account should exist");

    let proposal_data = RoleProposal::try_deserialize(&mut proposal_account.data.as_slice())
        .expect("Failed to deserialize proposal account");

    assert_eq!(proposal_data.roles, Roles::OPERATOR);
}
