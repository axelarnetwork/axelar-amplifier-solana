#![cfg(test)]
#![allow(clippy::too_many_lines)]

use anchor_spl::associated_token::get_associated_token_address_with_program_id;
use anchor_spl::token_2022::spl_token_2022;
use anchor_spl::token_2022::spl_token_2022::extension::StateWithExtensions;
use anchor_spl::token_2022::spl_token_2022::state::Account as TokenAccount;
use mollusk_svm::result::Check;
use mollusk_test_utils::setup_mollusk;
use solana_axelar_gateway::seed_prefixes::GATEWAY_SEED;
use solana_axelar_gateway::ID as GATEWAY_PROGRAM_ID;
use solana_axelar_gateway_test_fixtures::initialize_gateway;
use solana_axelar_gateway_test_fixtures::setup_test_with_real_signers;
use solana_axelar_its::state::TokenManager;
use solana_axelar_its::utils::canonical_interchain_token_id;
use solana_axelar_its_test_fixtures::setup_operator;
use solana_axelar_its_test_fixtures::{
    deploy_interchain_token_helper, DeployInterchainTokenContext,
};
use solana_axelar_its_test_fixtures::{
    init_gas_service, register_canonical_interchain_token_helper,
};
use solana_axelar_its_test_fixtures::{
    init_its_service_with_ethereum_trusted, perform_interchain_transfer,
};
use solana_axelar_its_test_fixtures::{initialize_mollusk, InterchainTransferContext};
use solana_sdk::program_pack::Pack;
use solana_sdk::rent::Rent;
use solana_sdk::signature::Keypair;
use solana_sdk::signer::Signer;
use solana_sdk::{account::Account, native_token::LAMPORTS_PER_SOL, pubkey::Pubkey};

#[test]
fn test_interchain_transfer_mint_burn() {
    let (setup, _, _, _, _) = setup_test_with_real_signers();

    let init_result = initialize_gateway(&setup);
    assert!(init_result.program_result.is_ok());

    let gas_service_program_id = solana_axelar_gas_service::id();
    let mut mollusk = setup_mollusk(&gas_service_program_id, "solana_axelar_gas_service");

    let operator = Pubkey::new_unique();
    let operator_account = Account::new(1_000_000_000, 0, &solana_sdk::system_program::ID);

    let (operator_pda, operator_pda_account) =
        setup_operator(&mut mollusk, operator, &operator_account);

    let (treasury_pda, treasury_pda_account) = init_gas_service(
        &mollusk,
        operator,
        &operator_account,
        operator_pda,
        &operator_pda_account,
    );

    let (gateway_root_pda, _) = Pubkey::find_program_address(&[GATEWAY_SEED], &GATEWAY_PROGRAM_ID);
    let gateway_root_pda_account = init_result.get_account(&gateway_root_pda).unwrap();

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

    let (its_root_pda, its_root_account) = init_its_service_with_ethereum_trusted(
        &mollusk,
        payer,
        &payer_account,
        payer,
        operator,
        &operator_account,
        chain_name.clone(),
        its_hub_address.clone(),
    );

    let salt = [1u8; 32];
    let name = "Test Token".to_owned();
    let symbol = "TEST".to_owned();
    let decimals = 9u8;
    let initial_supply = 1_000_000_000u64;

    let ctx = DeployInterchainTokenContext::new(
        mollusk,
        its_root_pda,
        its_root_account,
        deployer,
        deployer_account,
        program_id,
        payer,
        payer_account,
        None,
        None,
    );

    let (
        result,
        token_manager_pda,
        token_mint_pda,
        token_manager_ata,
        deployer_ata,
        _metadata_account,
        mollusk,
    ) = deploy_interchain_token_helper(
        salt,
        name.clone(),
        symbol.clone(),
        decimals,
        initial_supply,
        ctx,
    );

    assert!(result.program_result.is_ok());

    let source = deployer;
    let token_id = solana_axelar_its::utils::interchain_token_id(&deployer, &salt);
    let destination_chain = "ethereum".to_owned();
    let destination_address = b"0x1234567890123456789012345678901234567890".to_vec();
    let transfer_amount = 1_000_000u64;
    let gas_value = 0u64;

    let payer_account = result.get_account(&payer).unwrap().clone();
    let source_account = result.get_account(&source).unwrap().clone();
    let its_root_pda_account = result.get_account(&its_root_pda).unwrap().clone();
    let deployer_ata_account = result.get_account(&deployer_ata).unwrap().clone();
    let token_mint_pda_account = result.get_account(&token_mint_pda).unwrap().clone();
    let token_manager_pda_account = result.get_account(&token_manager_pda).unwrap().clone();
    let token_manager_ata_account = result.get_account(&token_manager_ata).unwrap().clone();

    let ctx = InterchainTransferContext::new(
        (payer, payer_account),
        (source, source_account),
        (its_root_pda, its_root_pda_account),
        (deployer_ata, deployer_ata_account),
        (token_mint_pda, token_mint_pda_account),
        (token_manager_pda, token_manager_pda_account),
        (token_manager_ata, token_manager_ata_account),
        (gateway_root_pda, gateway_root_pda_account.clone()),
        (treasury_pda, treasury_pda_account),
        mollusk,
    );

    let (transfer_result, _) = perform_interchain_transfer(
        ctx,
        token_id,
        destination_chain,
        destination_address,
        transfer_amount,
        gas_value,
    );

    assert!(transfer_result.program_result.is_ok());

    let deployer_ata_account = transfer_result.get_account(&deployer_ata).unwrap();

    // Parse the Token2022 account data to get the current balance
    let token_account = StateWithExtensions::<TokenAccount>::unpack(&deployer_ata_account.data)
        .expect("Failed to unpack source ATA data");

    let expected_balance = initial_supply - transfer_amount;
    assert_eq!(token_account.base.amount, expected_balance);
}

#[test]
fn test_interchain_transfer_lock_unlock() {
    let (setup, _, _, _, _) = setup_test_with_real_signers();

    let init_result = initialize_gateway(&setup);
    assert!(init_result.program_result.is_ok());

    let gas_service_program_id = solana_axelar_gas_service::id();
    let mut mollusk = setup_mollusk(&gas_service_program_id, "solana_axelar_gas_service");

    let operator = Pubkey::new_unique();
    let operator_account = Account::new(1_000_000_000, 0, &solana_sdk::system_program::ID);

    let (operator_pda, operator_pda_account) =
        setup_operator(&mut mollusk, operator, &operator_account);

    let (treasury_pda, treasury_pda_account) = init_gas_service(
        &mollusk,
        operator,
        &operator_account,
        operator_pda,
        &operator_pda_account,
    );

    let (gateway_root_pda, _) = Pubkey::find_program_address(&[GATEWAY_SEED], &GATEWAY_PROGRAM_ID);
    let gateway_root_pda_account = init_result.get_account(&gateway_root_pda).unwrap();

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

    let (its_root_pda, its_root_account) = init_its_service_with_ethereum_trusted(
        &mollusk,
        payer,
        &payer_account,
        payer,
        operator,
        &operator_account,
        chain_name.clone(),
        its_hub_address.clone(),
    );

    let mint_keypair = Keypair::new();
    let mint_pubkey = mint_keypair.pubkey();
    let mint_authority = Keypair::new();
    let initial_supply = 1_000_000_000; // 1 billion tokens

    // Create a basic SPL token mint
    let mint_data = {
        let mut data = vec![0u8; spl_token_2022::state::Mint::LEN];
        let mint = spl_token_2022::state::Mint {
            mint_authority: Some(mint_authority.pubkey()).into(),
            supply: initial_supply,
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
        payer,
        &payer_account,
        its_root_pda,
        &its_root_account,
        program_id,
        vec![Check::success()],
    );

    assert!(result.program_result.is_ok());

    let token_id = canonical_interchain_token_id(&mint_pubkey);
    let (token_manager_pda, _) = TokenManager::find_pda(token_id, its_root_pda);

    let token_manager_ata = get_associated_token_address_with_program_id(
        &token_manager_pda,
        &mint_pubkey,
        &spl_token_2022::ID,
    );

    let deployer_ata =
        get_associated_token_address_with_program_id(&deployer, &mint_pubkey, &spl_token_2022::ID);

    // Create the deployer's ATA with some tokens
    let deployer_ata_data = {
        let mut data = vec![0u8; spl_token_2022::state::Account::LEN];
        let token_account = spl_token_2022::state::Account {
            mint: mint_pubkey,
            owner: deployer,
            amount: initial_supply, // Give deployer all the tokens initially
            delegate: None.into(),
            state: spl_token_2022::state::AccountState::Initialized,
            is_native: None.into(),
            delegated_amount: 0,
            close_authority: None.into(),
        };
        spl_token_2022::state::Account::pack(token_account, &mut data).unwrap();
        data
    };

    let deployer_ata_account = Account {
        lamports: Rent::default().minimum_balance(spl_token_2022::state::Account::LEN),
        data: deployer_ata_data,
        owner: spl_token_2022::ID,
        executable: false,
        rent_epoch: 0,
    };

    let source = deployer;
    let destination_chain = "ethereum".to_owned();
    let destination_address = b"0x1234567890123456789012345678901234567890".to_vec();
    let transfer_amount = 1_000_000u64;
    let gas_value = 0u64;

    let payer_account = result.get_account(&payer).unwrap().clone();
    let its_root_pda_account = result.get_account(&its_root_pda).unwrap().clone();
    let mint_pubkey_account = result.get_account(&mint_pubkey).unwrap().clone();
    let token_manager_pda_account = result.get_account(&token_manager_pda).unwrap().clone();
    let token_manager_ata_account = result.get_account(&token_manager_ata).unwrap().clone();

    let ctx = InterchainTransferContext::new(
        (payer, payer_account),
        (source, deployer_account),
        (its_root_pda, its_root_pda_account),
        (deployer_ata, deployer_ata_account),
        (mint_pubkey, mint_pubkey_account),
        (token_manager_pda, token_manager_pda_account),
        (token_manager_ata, token_manager_ata_account),
        (gateway_root_pda, gateway_root_pda_account.clone()),
        (treasury_pda, treasury_pda_account),
        mollusk,
    );

    let (transfer_result, _) = perform_interchain_transfer(
        ctx,
        token_id,
        destination_chain,
        destination_address,
        transfer_amount,
        gas_value,
    );

    assert!(transfer_result.program_result.is_ok());

    let deployer_ata_account = transfer_result.get_account(&deployer_ata).unwrap();

    // Parse the Token2022 account data to get the current balance
    let token_account = StateWithExtensions::<TokenAccount>::unpack(&deployer_ata_account.data)
        .expect("Failed to unpack source ATA data");

    let expected_balance = initial_supply - transfer_amount;
    assert_eq!(token_account.base.amount, expected_balance);
}
