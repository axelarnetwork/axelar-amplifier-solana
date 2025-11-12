#![cfg(test)]
#![allow(clippy::too_many_lines)]

use anchor_lang::{
    AccountDeserialize, AnchorSerialize, Discriminator, InstructionData, ToAccountMetas,
};
use anchor_spl::associated_token::get_associated_token_address_with_program_id;
use anchor_spl::token_2022::spl_token_2022::{self, extension::StateWithExtensions};
use mollusk_svm::result::Check;
use solana_axelar_its::state::{Roles, RolesError, TokenManager, UserRoles};
use solana_axelar_its_test_fixtures::{
    deploy_interchain_token_helper, init_its_service, initialize_mollusk,
    DeployInterchainTokenContext,
};
use solana_sdk::program_pack::Pack;
use solana_sdk::{
    account::Account, instruction::Instruction, native_token::LAMPORTS_PER_SOL, pubkey::Pubkey,
    rent::Rent,
};
use spl_token_2022::state::Account as Token2022Account;

#[test]
fn test_deploy_and_mint_interchain_token() {
    let program_id = solana_axelar_its::id();
    let mollusk = initialize_mollusk();

    let payer = Pubkey::new_unique();
    let payer_account = Account::new(10 * LAMPORTS_PER_SOL, 0, &solana_sdk::system_program::ID);

    let deployer = Pubkey::new_unique();
    let deployer_account = Account::new(10 * LAMPORTS_PER_SOL, 0, &solana_sdk::system_program::ID);

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

    // Deploy an interchain token with a minter
    let salt = [1u8; 32];
    let name = "Mintable Token".to_owned();
    let symbol = "MINT".to_owned();
    let decimals = 9u8;
    let initial_supply = 1_000_000_000u64; // 1 billion tokens

    let minter = Pubkey::new_unique();

    let token_id = solana_axelar_its::utils::interchain_token_id(&deployer, &salt);

    let (token_manager_pda, _) = TokenManager::find_pda(token_id, its_root_pda);
    let (minter_roles_pda, _) = UserRoles::find_pda(&token_manager_pda, &minter);

    let ctx = DeployInterchainTokenContext::new(
        mollusk,
        (its_root_pda, its_root_account),
        (deployer, deployer_account),
        (payer, payer_account),
        Some(minter),
        Some(minter_roles_pda),
    );

    let (deploy_result, token_manager_pda, token_mint_pda, _, _, _metadata_account, mollusk) =
        deploy_interchain_token_helper(
            ctx,
            salt,
            name.clone(),
            symbol.clone(),
            decimals,
            initial_supply,
        );

    assert!(deploy_result.program_result.is_ok());

    // Verify the minter has the correct roles
    let minter_roles_account = deploy_result.get_account(&minter_roles_pda).unwrap();
    let minter_roles = UserRoles::try_deserialize(&mut minter_roles_account.data.as_ref()).unwrap();
    assert!(minter_roles.has_minter_role());

    // Create a random destination account and ATA
    let destination = Pubkey::new_unique();
    let destination_account = get_associated_token_address_with_program_id(
        &destination,
        &token_mint_pda,
        &spl_token_2022::ID,
    );

    // Create the destination ATA account (empty initially)
    let destination_ata_data = {
        let mut data = vec![0u8; spl_token_2022::state::Account::LEN];
        let token_account = spl_token_2022::state::Account {
            mint: token_mint_pda,
            owner: destination,
            amount: 0, // Start with 0 tokens
            delegate: None.into(),
            state: spl_token_2022::state::AccountState::Initialized,
            is_native: None.into(),
            delegated_amount: 0,
            close_authority: None.into(),
        };
        spl_token_2022::state::Account::pack(token_account, &mut data).unwrap();
        data
    };

    let destination_ata_account = Account {
        lamports: Rent::default().minimum_balance(spl_token_2022::state::Account::LEN),
        data: destination_ata_data,
        owner: spl_token_2022::ID,
        executable: false,
        rent_epoch: 0,
    };

    // Mint tokens to the destination using MintInterchainToken instruction
    let mint_amount = 50_000_000u64; // 50 million tokens

    let mint_instruction_data = solana_axelar_its::instruction::MintInterchainToken {
        amount: mint_amount,
    };

    let mint_accounts = solana_axelar_its::accounts::MintInterchainToken {
        mint: token_mint_pda,
        destination_account,
        its_root_pda,
        token_manager_pda,
        minter,
        minter_roles_pda,
        token_program: spl_token_2022::ID,
    };

    let mint_instruction = Instruction {
        program_id,
        accounts: mint_accounts.to_account_metas(None),
        data: mint_instruction_data.data(),
    };

    // Get updated accounts from deploy result
    let its_root_account = deploy_result.get_account(&its_root_pda).unwrap().clone();
    let token_manager_account = deploy_result
        .get_account(&token_manager_pda)
        .unwrap()
        .clone();
    let token_mint_account = deploy_result.get_account(&token_mint_pda).unwrap().clone();
    let minter_roles_account = deploy_result
        .get_account(&minter_roles_pda)
        .unwrap()
        .clone();

    // Create minter account
    let minter_account = Account::new(LAMPORTS_PER_SOL, 0, &solana_sdk::system_program::ID);

    let mint_accounts_vec = vec![
        (token_mint_pda, token_mint_account),
        (destination_account, destination_ata_account),
        (its_root_pda, its_root_account),
        (token_manager_pda, token_manager_account),
        (minter, minter_account),
        (minter_roles_pda, minter_roles_account),
        mollusk_svm_programs_token::token2022::keyed_account(),
    ];

    let mint_result = mollusk.process_instruction(&mint_instruction, &mint_accounts_vec);

    assert!(mint_result.program_result.is_ok());

    let destination_ata_account_after = mint_result.get_account(&destination_account).unwrap();
    let destination_account_data_after =
        StateWithExtensions::<Token2022Account>::unpack(&destination_ata_account_after.data)
            .unwrap();

    // verify user got the amount
    assert_eq!(destination_account_data_after.base.amount, mint_amount,);
    assert_eq!(destination_account_data_after.base.mint, token_mint_pda,);
    assert_eq!(destination_account_data_after.base.owner, destination);

    // Verify the mint supply increased
    let token_mint_account_after = mint_result.get_account(&token_mint_pda).unwrap();
    let token_mint_after =
        spl_token_2022::state::Mint::unpack(&token_mint_account_after.data).unwrap();

    assert_eq!(token_mint_after.supply, initial_supply + mint_amount);
}

#[test]
fn test_reject_mint_interchain_token_invalid_authority() {
    let program_id = solana_axelar_its::id();
    let mollusk = initialize_mollusk();

    let payer = Pubkey::new_unique();
    let payer_account = Account::new(10 * LAMPORTS_PER_SOL, 0, &solana_sdk::system_program::ID);

    let deployer = Pubkey::new_unique();
    let deployer_account = Account::new(10 * LAMPORTS_PER_SOL, 0, &solana_sdk::system_program::ID);

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

    // Deploy an interchain token with a minter
    let salt = [1u8; 32];
    let name = "Mintable Token".to_owned();
    let symbol = "MINT".to_owned();
    let decimals = 9u8;
    let initial_supply = 1_000_000_000u64; // 1 billion tokens

    let minter = Pubkey::new_unique();

    let token_id = solana_axelar_its::utils::interchain_token_id(&deployer, &salt);

    let (token_manager_pda, _) = TokenManager::find_pda(token_id, its_root_pda);
    let (minter_roles_pda, _) = UserRoles::find_pda(&token_manager_pda, &minter);

    let ctx = DeployInterchainTokenContext::new(
        mollusk,
        (its_root_pda, its_root_account),
        (deployer, deployer_account),
        (payer, payer_account),
        Some(minter),
        Some(minter_roles_pda),
    );

    let (deploy_result, token_manager_pda, token_mint_pda, _, _, _metadata_account, mollusk) =
        deploy_interchain_token_helper(
            ctx,
            salt,
            name.clone(),
            symbol.clone(),
            decimals,
            initial_supply,
        );

    assert!(deploy_result.program_result.is_ok());

    // Verify the minter has the correct roles
    let minter_roles_account = deploy_result.get_account(&minter_roles_pda).unwrap();
    let minter_roles = UserRoles::try_deserialize(&mut minter_roles_account.data.as_ref()).unwrap();
    assert!(minter_roles.has_minter_role());

    // Create a random destination account and ATA
    let destination = Pubkey::new_unique();
    let destination_account = get_associated_token_address_with_program_id(
        &destination,
        &token_mint_pda,
        &spl_token_2022::ID,
    );

    // Create the destination ATA account (empty initially)
    let destination_ata_data = {
        let mut data = vec![0u8; spl_token_2022::state::Account::LEN];
        let token_account = spl_token_2022::state::Account {
            mint: token_mint_pda,
            owner: destination,
            amount: 0, // Start with 0 tokens
            delegate: None.into(),
            state: spl_token_2022::state::AccountState::Initialized,
            is_native: None.into(),
            delegated_amount: 0,
            close_authority: None.into(),
        };
        spl_token_2022::state::Account::pack(token_account, &mut data).unwrap();
        data
    };

    let destination_ata_account = Account {
        lamports: Rent::default().minimum_balance(spl_token_2022::state::Account::LEN),
        data: destination_ata_data,
        owner: spl_token_2022::ID,
        executable: false,
        rent_epoch: 0,
    };

    // Mint tokens to the destination using MintInterchainToken instruction
    let mint_amount = 50_000_000u64; // 50 million tokens

    let mint_instruction_data = solana_axelar_its::instruction::MintInterchainToken {
        amount: mint_amount,
    };

    let unauthorized_minter = Pubkey::new_unique();

    let mint_accounts = solana_axelar_its::accounts::MintInterchainToken {
        mint: token_mint_pda,
        destination_account,
        its_root_pda,
        token_manager_pda,
        minter: unauthorized_minter,
        minter_roles_pda,
        token_program: spl_token_2022::ID,
    };

    let mint_instruction = Instruction {
        program_id,
        accounts: mint_accounts.to_account_metas(None),
        data: mint_instruction_data.data(),
    };

    // Get updated accounts from deploy result
    let its_root_account = deploy_result.get_account(&its_root_pda).unwrap().clone();
    let token_manager_account = deploy_result
        .get_account(&token_manager_pda)
        .unwrap()
        .clone();
    let token_mint_account = deploy_result.get_account(&token_mint_pda).unwrap().clone();
    let minter_roles_account = deploy_result
        .get_account(&minter_roles_pda)
        .unwrap()
        .clone();

    // Create minter account
    let minter_account = Account::new(LAMPORTS_PER_SOL, 0, &solana_sdk::system_program::ID);

    let mint_accounts = vec![
        (token_mint_pda, token_mint_account),
        (destination_account, destination_ata_account),
        (its_root_pda, its_root_account),
        (token_manager_pda, token_manager_account),
        (unauthorized_minter, minter_account),
        (minter_roles_pda, minter_roles_account),
        mollusk_svm_programs_token::token2022::keyed_account(),
    ];

    let checks = vec![Check::err(
        anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::ConstraintSeeds).into(),
    )];

    mollusk.process_and_validate_instruction(&mint_instruction, &mint_accounts, &checks);
}

#[test]
fn test_reject_mint_interchain_token_with_no_minter_role() {
    let program_id = solana_axelar_its::id();
    let mollusk = initialize_mollusk();

    let payer = Pubkey::new_unique();
    let payer_account = Account::new(10 * LAMPORTS_PER_SOL, 0, &solana_sdk::system_program::ID);

    let deployer = Pubkey::new_unique();
    let deployer_account = Account::new(10 * LAMPORTS_PER_SOL, 0, &solana_sdk::system_program::ID);

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

    // Deploy an interchain token with a minter
    let salt = [1u8; 32];
    let name = "Mintable Token".to_owned();
    let symbol = "MINT".to_owned();
    let decimals = 9u8;
    let initial_supply = 1_000_000_000u64; // 1 billion tokens

    let minter = Pubkey::new_unique();

    let token_id = solana_axelar_its::utils::interchain_token_id(&deployer, &salt);

    let (token_manager_pda, _) = TokenManager::find_pda(token_id, its_root_pda);
    let (minter_roles_pda, _) = UserRoles::find_pda(&token_manager_pda, &minter);

    let ctx = DeployInterchainTokenContext::new(
        mollusk,
        (its_root_pda, its_root_account),
        (deployer, deployer_account),
        (payer, payer_account),
        Some(minter),
        Some(minter_roles_pda),
    );

    let (deploy_result, token_manager_pda, token_mint_pda, _, _, _metadata_account, mollusk) =
        deploy_interchain_token_helper(
            ctx,
            salt,
            name.clone(),
            symbol.clone(),
            decimals,
            initial_supply,
        );

    assert!(deploy_result.program_result.is_ok());

    // Verify the minter has the correct roles
    let minter_roles_account = deploy_result.get_account(&minter_roles_pda).unwrap();
    let minter_roles = UserRoles::try_deserialize(&mut minter_roles_account.data.as_ref()).unwrap();
    assert!(minter_roles.has_minter_role());

    // Create a random destination account and ATA
    let destination = Pubkey::new_unique();
    let destination_account = get_associated_token_address_with_program_id(
        &destination,
        &token_mint_pda,
        &spl_token_2022::ID,
    );

    // Create the destination ATA account (empty initially)
    let destination_ata_data = {
        let mut data = vec![0u8; spl_token_2022::state::Account::LEN];
        let token_account = spl_token_2022::state::Account {
            mint: token_mint_pda,
            owner: destination,
            amount: 0, // Start with 0 tokens
            delegate: None.into(),
            state: spl_token_2022::state::AccountState::Initialized,
            is_native: None.into(),
            delegated_amount: 0,
            close_authority: None.into(),
        };
        spl_token_2022::state::Account::pack(token_account, &mut data).unwrap();
        data
    };

    let destination_ata_account = Account {
        lamports: Rent::default().minimum_balance(spl_token_2022::state::Account::LEN),
        data: destination_ata_data,
        owner: spl_token_2022::ID,
        executable: false,
        rent_epoch: 0,
    };

    // Mint tokens to the destination using MintInterchainToken instruction
    let mint_amount = 50_000_000u64; // 50 million tokens

    let mint_instruction_data = solana_axelar_its::instruction::MintInterchainToken {
        amount: mint_amount,
    };

    let mint_accounts = solana_axelar_its::accounts::MintInterchainToken {
        mint: token_mint_pda,
        destination_account,
        its_root_pda,
        token_manager_pda,
        minter,
        minter_roles_pda,
        token_program: spl_token_2022::ID,
    };

    let mint_instruction = Instruction {
        program_id,
        accounts: mint_accounts.to_account_metas(None),
        data: mint_instruction_data.data(),
    };

    // Get updated accounts from deploy result
    let its_root_account = deploy_result.get_account(&its_root_pda).unwrap().clone();
    let token_manager_account = deploy_result
        .get_account(&token_manager_pda)
        .unwrap()
        .clone();
    let token_mint_account = deploy_result.get_account(&token_mint_pda).unwrap().clone();
    let minter_roles_account = deploy_result
        .get_account(&minter_roles_pda)
        .unwrap()
        .clone();

    // Remove roles from minter account
    let mut minter_roles_account_clone = minter_roles_account.clone();

    let mut minter_roles =
        UserRoles::try_deserialize(&mut minter_roles_account_clone.data.as_ref())
            .expect("Failed to deserialize flow limiter roles");
    minter_roles.roles = Roles::empty();

    let mut new_data = Vec::new();
    new_data.extend_from_slice(UserRoles::DISCRIMINATOR);
    minter_roles
        .serialize(&mut new_data)
        .expect("Failed to serialize");
    minter_roles_account_clone.data = new_data;

    // Create minter account
    let minter_account = Account::new(LAMPORTS_PER_SOL, 0, &solana_sdk::system_program::ID);

    let mint_accounts_vec = vec![
        (token_mint_pda, token_mint_account),
        (destination_account, destination_ata_account),
        (its_root_pda, its_root_account),
        (token_manager_pda, token_manager_account),
        (minter, minter_account),
        (minter_roles_pda, minter_roles_account_clone),
        mollusk_svm_programs_token::token2022::keyed_account(),
    ];

    let checks = vec![Check::err(
        anchor_lang::error::Error::from(RolesError::MissingMinterRole).into(),
    )];

    mollusk.process_and_validate_instruction(&mint_instruction, &mint_accounts_vec, &checks);
}
