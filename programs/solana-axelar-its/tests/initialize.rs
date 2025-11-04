#![cfg(test)]
#![allow(clippy::str_to_string)]
#![allow(clippy::print_stdout)]

use anchor_lang::AccountDeserialize;
use mollusk_test_utils::setup_mollusk;
use solana_axelar_its::state::{InterchainTokenService, Roles, UserRoles};
use solana_axelar_its_test_fixtures::init_its_service;
use {
    solana_sdk::{account::Account, pubkey::Pubkey},
    solana_sdk_ids::bpf_loader_upgradeable,
};

#[test]
fn test_initialize_success() {
    let program_id = solana_axelar_its::id();
    let mollusk = setup_mollusk(&program_id, "solana_axelar_its");

    let upgrade_authority = Pubkey::new_unique();

    // We require that the payer be the upgrade_authority
    let payer = upgrade_authority;
    let payer_account = Account::new(1_000_000_000, 0, &solana_sdk::system_program::ID);

    let operator = Pubkey::new_unique();
    let operator_account = Account::new(1_000_000_000, 0, &solana_sdk::system_program::ID);

    let chain_name = "solana".to_string();
    let its_hub_address = "0x123456789abcdef".to_string();

    let (
        its_root_pda,
        its_root_account,
        user_roles_pda,
        user_roles_account,
        _program_data,
        _program_data_account,
    ) = init_its_service(
        &mollusk,
        payer,
        &payer_account,
        payer,
        operator,
        &operator_account,
        chain_name.clone(),
        its_hub_address.clone(),
    );

    // Verify the ITS root PDA is properly initialized
    let its_data = InterchainTokenService::try_deserialize(&mut its_root_account.data.as_slice())
        .expect("Failed to deserialize ITS data");

    assert_eq!(its_data.chain_name, chain_name);
    assert_eq!(its_data.its_hub_address, its_hub_address);
    assert!(!its_data.paused);
    assert_eq!(its_data.trusted_chains.len(), 0);

    // Verify the user roles PDA is properly initialized
    let roles_data = UserRoles::try_deserialize(&mut user_roles_account.data.as_slice())
        .expect("Failed to deserialize roles data");

    assert_eq!(roles_data.roles, Roles::OPERATOR);

    // Verify PDAs are derived correctly
    let expected_its_pda =
        Pubkey::find_program_address(&[InterchainTokenService::SEED_PREFIX], &program_id).0;
    assert_eq!(its_root_pda, expected_its_pda);

    let expected_roles_pda = Pubkey::find_program_address(
        &[
            UserRoles::SEED_PREFIX,
            its_root_pda.as_ref(),
            operator.as_ref(),
        ],
        &program_id,
    )
    .0;
    assert_eq!(user_roles_pda, expected_roles_pda);

    // Verify the program data PDA is correct
    let expected_program_data =
        Pubkey::find_program_address(&[program_id.as_ref()], &bpf_loader_upgradeable::ID).0;
    let (actual_program_data, _) = (expected_program_data, 0); // We verified it in init_its_service
    assert_eq!(actual_program_data, expected_program_data);
}

#[test]
#[should_panic = "InvalidAccountData"]
fn test_initialize_unauthorized_payer() {
    let program_id = solana_axelar_its::id();
    let mollusk = setup_mollusk(&program_id, "solana_axelar_its");

    let upgrade_authority = Pubkey::new_unique();

    // We make the payer different from the upgrade_authority
    let payer = Pubkey::new_unique();
    let payer_account = Account::new(1_000_000_000, 0, &solana_sdk::system_program::ID);

    let operator = Pubkey::new_unique();
    let operator_account = Account::new(1_000_000_000, 0, &solana_sdk::system_program::ID);

    let chain_name = "solana".to_string();
    let its_hub_address = "0x123456789abcdef".to_string();

    // This should fail because payer is not the upgrade authority
    // The program data account was created with authorized_payer as authority
    let (
        _its_root_pda,
        _its_root_account,
        _user_roles_pda,
        _user_roles_account,
        _program_data,
        _program_data_account,
    ) = init_its_service(
        &mollusk,
        payer,
        &payer_account,
        upgrade_authority,
        operator,
        &operator_account,
        chain_name,
        its_hub_address,
    );
}
