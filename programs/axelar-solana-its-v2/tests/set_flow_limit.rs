use anchor_lang::{AccountDeserialize, InstructionData, ToAccountMetas};
use axelar_solana_its_v2::{
    state::{TokenManager, UserRoles},
    utils::interchain_token_id,
};
use axelar_solana_its_v2_test_fixtures::{
    deploy_interchain_token_helper, DeployInterchainTokenContext,
};
use mollusk_svm::program::keyed_account_for_system_program;
use mollusk_test_utils::get_event_authority_and_program_accounts;
use solana_program::instruction::Instruction;
use solana_sdk::{account::Account, native_token::LAMPORTS_PER_SOL, pubkey::Pubkey};

#[path = "initialize.rs"]
mod initialize;

#[test]
fn test_set_flow_limit_success() {
    let program_id = axelar_solana_its_v2::id();
    let mollusk = initialize::initialize_mollusk();

    let payer = Pubkey::new_unique();
    let payer_account = Account::new(10 * LAMPORTS_PER_SOL, 0, &solana_sdk::system_program::ID);

    let deployer = Pubkey::new_unique();
    let deployer_account = Account::new(10 * LAMPORTS_PER_SOL, 0, &solana_sdk::system_program::ID);

    let operator = Pubkey::new_unique();
    let operator_account = Account::new(1_000_000_000, 0, &solana_sdk::system_program::ID);

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

    // Deploy an interchain token first
    let salt = [1u8; 32];
    let name = "Test Token".to_string();
    let symbol = "TEST".to_string();
    let decimals = 9u8;
    let initial_supply = 1_000_000_000u64; // 1 billion tokens with 9 decimals
    let minter = Pubkey::new_unique();

    let token_id = interchain_token_id(&deployer, &salt);
    let (token_manager_pda, _token_manager_bump) = TokenManager::find_pda(token_id, its_root_pda);
    let (minter_roles_pda, _) = UserRoles::find_pda(&token_manager_pda, &minter);

    let ctx = DeployInterchainTokenContext::new(
        mollusk,
        its_root_pda,
        its_root_account,
        deployer,
        deployer_account,
        program_id,
        payer,
        payer_account.clone(),
        Some(minter),
        Some(minter_roles_pda),
    );

    let (
        result,
        token_manager_pda,
        _token_mint_pda,
        _token_manager_ata,
        _deployer_ata,
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

    assert!(
        result.program_result.is_ok(),
        "Deploy interchain token instruction should succeed: {:?}",
        result.program_result
    );

    // Verify the token manager was created with no flow limit initially
    let token_manager_account = result.get_account(&token_manager_pda).unwrap();
    let token_manager =
        TokenManager::try_deserialize(&mut token_manager_account.data.as_ref()).unwrap();

    assert_eq!(
        token_manager.flow_slot.flow_limit, None,
        "Initial flow limit should be None"
    );

    // Now set the flow limit
    let flow_limit: Option<u64> = Some(1_000_000_000); // 1 billion tokens
    let (operator_roles_pda, _) = UserRoles::find_pda(&token_manager_pda, &minter);

    let (event_authority, event_authority_account, program_account) =
        get_event_authority_and_program_accounts(&program_id);

    let ix = Instruction {
        program_id,
        accounts: axelar_solana_its_v2::accounts::SetFlowLimit {
            payer,
            operator: minter, // Using minter as operator
            its_root_pda,
            its_roles_pda: operator_roles_pda,
            token_manager_pda,
            system_program: solana_sdk::system_program::ID,
            // for emit cpi
            event_authority,
            program: program_id,
        }
        .to_account_metas(None),
        data: axelar_solana_its_v2::instruction::SetFlowLimit { flow_limit }.data(),
    };

    let updated_its_root_account = result.get_account(&its_root_pda).unwrap();
    let updated_token_manager_account = result.get_account(&token_manager_pda).unwrap();
    let operator_roles_account = result.get_account(&operator_roles_pda).unwrap();
    let operator_account = Account::new(1_000_000_000, 0, &solana_sdk::system_program::ID);

    let accounts = vec![
        (payer, payer_account),
        (minter, operator_account),
        (its_root_pda, updated_its_root_account.clone()),
        (operator_roles_pda, operator_roles_account.clone()),
        (token_manager_pda, updated_token_manager_account.clone()),
        keyed_account_for_system_program(),
        // for event cpi
        (event_authority, event_authority_account),
        (program_id, program_account),
    ];

    let result = mollusk.process_instruction(&ix, &accounts);

    let updated_token_manager_account = result.get_account(&token_manager_pda).unwrap();
    let updated_token_manager =
        TokenManager::try_deserialize(&mut updated_token_manager_account.data.as_ref()).unwrap();

    assert_eq!(
        updated_token_manager.flow_slot.flow_limit, flow_limit,
        "Flow limit should be set to the specified value"
    );

    assert_eq!(updated_token_manager.token_id, token_manager.token_id);
    assert_eq!(
        updated_token_manager.token_address,
        token_manager.token_address
    );
    assert_eq!(updated_token_manager.flow_slot.flow_in, 0);
    assert_eq!(updated_token_manager.flow_slot.flow_out, 0);
    assert_eq!(updated_token_manager.flow_slot.epoch, 0);
}
