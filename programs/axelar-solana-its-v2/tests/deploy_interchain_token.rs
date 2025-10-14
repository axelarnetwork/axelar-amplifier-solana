use anchor_lang::AccountDeserialize;
use anchor_spl::{
    token::spl_token,
    token_2022::spl_token_2022::{self, extension::StateWithExtensions},
};
use axelar_solana_its_v2::{
    state::{TokenManager, UserRoles},
    utils::{interchain_token_deployer_salt, interchain_token_id_internal},
};
use axelar_solana_its_v2_test_fixtures::{
    deploy_interchain_token_helper, DeployInterchainTokenContext,
};
use mollusk_svm_programs_token;
use mollusk_test_utils::setup_mollusk;
use solana_program::program_pack::Pack;
use solana_sdk::{
    account::Account, native_token::LAMPORTS_PER_SOL, pubkey::Pubkey, system_program,
};
use spl_token_2022::state::Account as Token2022Account;

#[path = "initialize.rs"]
mod initialize;

#[test]
fn test_deploy_interchain_token() {
    let program_id = axelar_solana_its_v2::id();
    let mollusk = initialize::initialize_mollusk();

    let payer = Pubkey::new_unique();
    let payer_account = Account::new(10 * LAMPORTS_PER_SOL, 0, &system_program::ID);

    let deployer = Pubkey::new_unique();
    let deployer_account = Account::new(10 * LAMPORTS_PER_SOL, 0, &system_program::ID);

    let operator = Pubkey::new_unique();
    let operator_account = Account::new(1_000_000_000, 0, &system_program::ID);

    let chain_name = "solana".to_string();
    let its_hub_address = "0x123456789abcdef".to_string();

    // Initialize ITS service first
    let (
        its_root_pda,
        its_root_account,
        _user_roles_pda,
        _user_roles_account,
        _program_data,
        _program_data_account,
    ) = initialize::init_its_service(
        &mollusk,
        payer,
        &payer_account,
        payer,
        operator,
        &operator_account,
        chain_name.clone(),
        its_hub_address.clone(),
    );

    // Create simple token deployment parameters
    let salt = [1u8; 32];
    let name = "Test Token".to_string();
    let symbol = "TEST".to_string();
    let decimals = 9u8;
    let initial_supply = 1_000_000_000u64; // 1 billion tokens with 9 decimals

    let minter = Pubkey::new_unique();

    let token_id = axelar_solana_its_v2::utils::interchain_token_id(&deployer, &salt);
    let (token_manager_pda, _token_manager_bump) = Pubkey::find_program_address(
        &[
            axelar_solana_its_v2::seed_prefixes::TOKEN_MANAGER_SEED,
            its_root_pda.as_ref(),
            &token_id,
        ],
        &program_id,
    );
    let (minter_roles_pda, minter_roles_pda_bump) = Pubkey::find_program_address(
        &[
            axelar_solana_its_v2::state::UserRoles::SEED_PREFIX,
            token_manager_pda.as_ref(),
            minter.as_ref(),
        ],
        &program_id,
    );

    let ctx = DeployInterchainTokenContext::new(
        mollusk,
        its_root_pda,
        its_root_account,
        deployer,
        deployer_account,
        program_id,
        payer,
        payer_account,
        Some(minter),
        Some(minter_roles_pda),
    );

    let (
        result,
        token_manager_pda,
        token_mint_pda,
        token_manager_ata,
        deployer_ata,
        metadata_account,
        _,
    ) = deploy_interchain_token_helper(
        salt,
        name.clone(),
        symbol.clone(),
        decimals,
        initial_supply,
        ctx,
    );

    assert!(
        result.program_result.is_ok(),
        "Deploy interchain token instruction should succeed: {:?}",
        result.program_result
    );

    let minter_roles_account = result.get_account(&minter_roles_pda).unwrap();
    let minter_roles = UserRoles::try_deserialize(&mut minter_roles_account.data.as_ref()).unwrap();
    // Minter gets all 3 roles
    assert!(minter_roles.has_minter_role());
    assert!(minter_roles.has_operator_role());
    assert!(minter_roles.has_flow_limiter_role());
    assert_eq!(minter_roles.bump, minter_roles_pda_bump);

    let token_manager_account = result.get_account(&token_manager_pda).unwrap();
    let token_manager =
        TokenManager::try_deserialize(&mut token_manager_account.data.as_ref()).unwrap();

    let deploy_salt = interchain_token_deployer_salt(&deployer, &salt);
    let expected_token_id = interchain_token_id_internal(&deploy_salt);

    assert_eq!(token_manager.token_id, expected_token_id);
    assert_eq!(token_manager.token_address, token_mint_pda);
    assert_eq!(token_manager.associated_token_account, token_manager_ata);
    assert_eq!(token_manager.flow_slot.flow_limit, None);
    assert_eq!(token_manager.flow_slot.flow_in, 0);
    assert_eq!(token_manager.flow_slot.flow_out, 0);
    assert_eq!(token_manager.flow_slot.epoch, 0);

    let token_mint_account = result.get_account(&token_mint_pda).unwrap();
    let token_mint = spl_token_2022::state::Mint::unpack(&token_mint_account.data).unwrap();
    assert_eq!(
        token_mint.mint_authority,
        Some(token_manager_pda).into(),
        "Mint authority should be the token manager PDA"
    );
    assert_eq!(
        token_mint.freeze_authority,
        Some(token_manager_pda).into(),
        "Freeze authority should be the token manager PDA"
    );
    assert_eq!(
        token_mint.supply, initial_supply,
        "Total supply should match initial supply"
    );

    let token_manager_ata_account = result.get_account(&token_manager_ata).unwrap();
    let token_manager_ata_data =
        StateWithExtensions::<Token2022Account>::unpack(&token_manager_ata_account.data).unwrap();
    assert_eq!(
        token_manager_ata_data.base.mint, token_mint_pda,
        "ATA mint should match the token mint PDA"
    );
    assert_eq!(
        token_manager_ata_data.base.owner, token_manager_pda,
        "ATA owner should be the token manager PDA"
    );
    assert_eq!(
        token_manager_ata_data.base.amount, 0,
        "Token Manager ATA should start with 0 tokens"
    );

    let deployer_ata_account = result.get_account(&deployer_ata).unwrap();
    let deployer_ata_data =
        StateWithExtensions::<Token2022Account>::unpack(&deployer_ata_account.data).unwrap();
    assert_eq!(
        deployer_ata_data.base.mint, token_mint_pda,
        "Deployer ATA mint should match the token mint PDA"
    );
    assert_eq!(
        deployer_ata_data.base.owner, deployer,
        "Deployer ATA owner should be the deployer"
    );
    assert_eq!(
        deployer_ata_data.base.amount, initial_supply,
        "Deployer ATA should have the initial supply"
    );

    let metadata_acc = result.get_account(&metadata_account).unwrap();
    let metadata = mpl_token_metadata::accounts::Metadata::from_bytes(&metadata_acc.data).unwrap();
    assert_eq!(
        metadata.mint, token_mint_pda,
        "Metadata mint should match the token mint PDA"
    );
    assert_eq!(
        metadata.update_authority, token_manager_pda,
        "Metadata update authority should be the token manager PDA"
    );

    // Check name and symbol (trim null bytes for comparison)
    let metadata_name = metadata.name.trim_end_matches('\0');
    let metadata_symbol = metadata.symbol.trim_end_matches('\0');

    assert_eq!(
        metadata_name, name,
        "Metadata name should match the input name"
    );
    assert_eq!(
        metadata_symbol, symbol,
        "Metadata symbol should match the input symbol"
    );
}
