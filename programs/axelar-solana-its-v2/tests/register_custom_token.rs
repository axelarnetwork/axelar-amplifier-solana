use anchor_lang::AccountDeserialize;
use anchor_spl::token_2022::spl_token_2022;
use axelar_solana_its_v2::{
    state::{token_manager::Type, TokenManager},
    utils::{interchain_token_id_internal, linked_token_deployer_salt},
};
use solana_program::program_pack::Pack;
use solana_sdk::{
    account::Account, instruction::Instruction, native_token::LAMPORTS_PER_SOL, pubkey::Pubkey,
    system_program,
};

#[path = "initialize.rs"]
mod initialize;

fn create_test_mint(mint_authority: Pubkey) -> (Pubkey, Account) {
    let mint = Pubkey::new_unique();
    let mint_data = {
        let mut data = [0u8; spl_token_2022::state::Mint::LEN];
        let mint_state = spl_token_2022::state::Mint {
            mint_authority: Some(mint_authority).into(),
            supply: 1_000_000_000, // 1 billion tokens
            decimals: 9,
            is_initialized: true,
            freeze_authority: Some(mint_authority).into(),
        };
        spl_token_2022::state::Mint::pack(mint_state, &mut data).unwrap();
        data.to_vec()
    };
    let rent = anchor_lang::prelude::Rent::default();
    let mint_account = Account {
        lamports: rent.minimum_balance(mint_data.len()),
        data: mint_data,
        owner: spl_token_2022::ID,
        executable: false,
        rent_epoch: 0,
    };

    (mint, mint_account)
}

#[test]
fn test_register_custom_token_without_operator() {
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

    // Create a test mint (existing token to register)
    let mint_authority = Pubkey::new_unique();
    let (token_mint, token_mint_account) = create_test_mint(mint_authority);

    // Register custom token parameters
    let salt = [2u8; 32];
    let token_manager_type = Type::LockUnlock;
    let operator_param: Option<Pubkey> = None; // No operator

    let token_id = {
        let deploy_salt = linked_token_deployer_salt(&deployer, &salt);
        interchain_token_id_internal(&deploy_salt)
    };

    let (token_manager_pda, _token_manager_bump) = TokenManager::find_pda(token_id, its_root_pda);
    let token_manager_ata =
        anchor_spl::associated_token::get_associated_token_address_with_program_id(
            &token_manager_pda,
            &token_mint,
            &spl_token_2022::ID,
        );

    // Create the instruction data
    use anchor_lang::{InstructionData, ToAccountMetas};
    let instruction_data = axelar_solana_its_v2::instruction::RegisterCustomToken {
        salt,
        token_manager_type,
        operator: operator_param,
    };

    let (event_authority, _event_authority_bump) =
        Pubkey::find_program_address(&[b"__event_authority"], &program_id);

    // Build account metas
    let accounts = axelar_solana_its_v2::accounts::RegisterCustomToken {
        payer,
        deployer,
        system_program: system_program::ID,
        its_root_pda,
        token_manager_pda,
        token_mint,
        token_manager_ata,
        token_program: spl_token_2022::ID,
        associated_token_program: anchor_spl::associated_token::ID,
        rent: solana_program::sysvar::rent::ID,
        operator: None,
        operator_roles_pda: None,
        // for event cpi
        event_authority,
        program: program_id,
    };

    let ix = Instruction {
        program_id,
        accounts: accounts.to_account_metas(None),
        data: instruction_data.data(),
    };

    // Set up accounts for mollusk
    let mollusk_accounts = vec![
        (payer, payer_account),
        (deployer, deployer_account),
        (
            system_program::ID,
            Account {
                lamports: 1,
                data: vec![],
                owner: solana_sdk::native_loader::id(),
                executable: true,
                rent_epoch: 0,
            },
        ),
        (its_root_pda, its_root_account),
        (token_manager_pda, Account::new(0, 0, &system_program::ID)),
        (token_mint, token_mint_account),
        (token_manager_ata, Account::new(0, 0, &system_program::ID)),
        mollusk_svm_programs_token::token2022::keyed_account(),
        mollusk_svm_programs_token::associated_token::keyed_account(),
        (
            anchor_spl::associated_token::ID,
            Account {
                lamports: 1,
                data: vec![],
                owner: solana_sdk::native_loader::id(),
                executable: true,
                rent_epoch: 0,
            },
        ),
        (
            solana_sdk::sysvar::rent::ID,
            Account {
                lamports: 1_000_000_000,
                data: {
                    let rent = anchor_lang::prelude::Rent::default();
                    bincode::serialize(&rent).unwrap()
                },
                owner: solana_sdk::sysvar::rent::ID,
                executable: false,
                rent_epoch: 0,
            },
        ),
        // For event CPI
        (event_authority, Account::new(0, 0, &system_program::ID)),
        (program_id, Account::new(0, 0, &system_program::ID)),
    ];

    let result = mollusk.process_and_validate_instruction(
        &ix,
        &mollusk_accounts,
        &[mollusk_svm::result::Check::success()],
    );

    assert!(
        result.program_result.is_ok(),
        "Register custom token instruction should succeed: {:?}",
        result.program_result
    );

    // Verify token manager was created correctly
    let token_manager_account = result.get_account(&token_manager_pda).unwrap();
    let token_manager =
        TokenManager::try_deserialize(&mut token_manager_account.data.as_ref()).unwrap();

    let expected_token_id = {
        let deploy_salt = linked_token_deployer_salt(&deployer, &salt);
        interchain_token_id_internal(&deploy_salt)
    };

    assert_eq!(token_manager.ty, Type::LockUnlock);
    assert_eq!(token_manager.token_id, expected_token_id);
    assert_eq!(token_manager.token_address, token_mint);
    assert_eq!(token_manager.associated_token_account, token_manager_ata);
    assert_eq!(token_manager.flow_slot.flow_limit, None);
}
