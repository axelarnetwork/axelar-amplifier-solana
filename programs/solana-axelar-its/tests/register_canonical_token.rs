#![cfg(test)]
#![allow(clippy::too_many_lines)]

use anchor_lang::prelude::ProgramError;
use anchor_lang::solana_program;
use anchor_lang::AccountDeserialize;
use anchor_spl::associated_token::get_associated_token_address_with_program_id;
use anchor_spl::token_2022::spl_token_2022::{self, extension::StateWithExtensions};
use mollusk_svm::result::Check;
use solana_axelar_its::{
    state::TokenManager,
    utils::{
        canonical_interchain_token_deploy_salt, canonical_interchain_token_id,
        interchain_token_id_internal,
    },
};
use solana_axelar_its_test_fixtures::{
    init_its_service, initialize_mollusk, register_canonical_interchain_token_helper,
};
use solana_program::program_pack::Pack;
use solana_sdk::{
    account::Account, native_token::LAMPORTS_PER_SOL, pubkey::Pubkey, signature::Keypair,
    signer::Signer,
};
use spl_token_2022::state::Account as Token2022Account;

#[test]
fn test_register_canonical_token() {
    let mollusk = initialize_mollusk();

    let payer = Pubkey::new_unique();
    let payer_account = Account::new(10 * LAMPORTS_PER_SOL, 0, &solana_sdk::system_program::ID);

    let operator = Pubkey::new_unique();
    let operator_account = Account::new(1_000_000_000, 0, &solana_sdk::system_program::ID);

    let chain_name = "solana".to_owned();
    let its_hub_address = "0x123456789abcdef".to_owned();

    // Initialize ITS service first
    let (
        its_root_pda,
        its_root_account,
        _user_roles_pda,
        _user_roles_account,
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

    // Create a token mint (this would be an existing token we want to register as canonical)
    let mint_keypair = Keypair::new();
    let mint_pubkey = mint_keypair.pubkey();
    let mint_authority = Keypair::new();

    // Create a basic SPL token mint
    let mint_data = {
        let mut data = vec![0u8; spl_token_2022::state::Mint::LEN];
        let mint = spl_token_2022::state::Mint {
            mint_authority: Some(mint_authority.pubkey()).into(),
            supply: 1_000_000_000, // 1 billion tokens
            decimals: 9,
            is_initialized: true,
            freeze_authority: Some(mint_authority.pubkey()).into(),
        };
        spl_token_2022::state::Mint::pack(mint, &mut data).unwrap();
        data
    };

    let result = register_canonical_interchain_token_helper(
        &mollusk,
        mint_data,
        &mint_keypair,
        &mint_authority,
        (payer, payer_account),
        (its_root_pda, its_root_account.clone()),
        vec![Check::success()],
    );

    assert!(
        result.program_result.is_ok(),
        "Register canonical token instruction should succeed: {:?}",
        result.program_result
    );

    let token_id = canonical_interchain_token_id(&mint_pubkey);
    let (token_manager_pda, _) = TokenManager::find_pda(token_id, its_root_pda);

    let token_manager_ata = get_associated_token_address_with_program_id(
        &token_manager_pda,
        &mint_pubkey,
        &spl_token_2022::ID,
    );

    // Verify token manager was created correctly
    let token_manager_account = result.get_account(&token_manager_pda).unwrap();
    let token_manager =
        TokenManager::try_deserialize(&mut token_manager_account.data.as_ref()).unwrap();

    let deploy_salt = canonical_interchain_token_deploy_salt(&mint_pubkey);
    let expected_token_id = interchain_token_id_internal(&deploy_salt);

    assert_eq!(token_manager.token_id, expected_token_id);
    assert_eq!(token_manager.token_address, mint_pubkey);
    assert_eq!(token_manager.associated_token_account, token_manager_ata);
    assert_eq!(token_manager.flow_slot.flow_limit, None);
    assert_eq!(token_manager.flow_slot.flow_in, 0);
    assert_eq!(token_manager.flow_slot.flow_out, 0);
    assert_eq!(token_manager.flow_slot.epoch, 0);
    assert_eq!(token_manager.ty, solana_axelar_its::state::Type::LockUnlock); // No fee extension

    // Verify token manager ATA was created
    let token_manager_ata_account = result.get_account(&token_manager_ata).unwrap();
    let token_manager_ata_data =
        StateWithExtensions::<Token2022Account>::unpack(&token_manager_ata_account.data).unwrap();

    assert_eq!(token_manager_ata_data.base.mint, mint_pubkey);
    assert_eq!(token_manager_ata_data.base.owner, token_manager_pda);
    assert_eq!(token_manager_ata_data.base.amount, 0);
}

#[test]
fn test_reject_register_canonical_token_with_invalid_metadata() {
    let mollusk = initialize_mollusk();

    let payer = Pubkey::new_unique();
    let payer_account = Account::new(10 * LAMPORTS_PER_SOL, 0, &solana_sdk::system_program::ID);

    let operator = Pubkey::new_unique();
    let operator_account = Account::new(1_000_000_000, 0, &solana_sdk::system_program::ID);

    let chain_name = "solana".to_owned();
    let its_hub_address = "0x123456789abcdef".to_owned();

    // Initialize ITS service first
    let (its_root_pda, its_root_account, _, _, _, _) = init_its_service(
        &mollusk,
        payer,
        &payer_account,
        payer,
        operator,
        &operator_account,
        chain_name.clone(),
        its_hub_address.clone(),
    );

    // Create a token mint (this would be an existing token we want to register as canonical)
    let mint_keypair = Keypair::new();
    let mint_authority = Keypair::new();

    let checks = vec![Check::err(
        anchor_lang::error::Error::from(ProgramError::InvalidAccountData).into(),
    )];

    let result = register_canonical_interchain_token_helper(
        &mollusk,
        vec![], // empty mint data
        &mint_keypair,
        &mint_authority,
        (payer, payer_account),
        (its_root_pda, its_root_account.clone()),
        checks,
    );

    assert!(result.program_result.is_err());
}
