#![cfg(test)]
#![allow(clippy::too_many_lines)]

use anchor_lang::{AccountDeserialize, InstructionData, ToAccountMetas};
use anchor_spl::token_2022::spl_token_2022;
use mollusk_svm::program::keyed_account_for_system_program;
use mollusk_svm::result::Check;
use solana_axelar_its::state::{Roles, Type, UserRoles};
use solana_axelar_its_test_fixtures::{
    create_test_mint, execute_register_custom_token_helper, init_its_service,
    initialize_mollusk_with_programs, new_empty_account, new_test_account,
    RegisterCustomTokenContext, RegisterCustomTokenParams,
};
use solana_program::program_pack::Pack;
use solana_sdk::instruction::Instruction;

#[test]
fn test_handover_mint_authority_success() {
    let mollusk = initialize_mollusk_with_programs();

    let (payer, payer_account) = new_test_account();
    let (deployer, deployer_account) = new_test_account();
    let (operator, operator_account) = new_test_account();
    let (mint_authority, mint_authority_account) = new_test_account();

    let chain_name = "solana".to_owned();
    let its_hub_address = "0x123456789abcdef".to_owned();

    // Initialize ITS service
    let (its_root_pda, its_root_account, _, _, _, _) = init_its_service(
        &mollusk,
        payer,
        &payer_account,
        payer,
        operator,
        &operator_account,
        chain_name,
        its_hub_address,
    );

    let (token_mint, token_mint_account) = create_test_mint(mint_authority);

    // Register the token as a custom token with MintBurn type
    let salt = [1u8; 32];
    let token_manager_type = Type::MintBurn;

    let ctx = RegisterCustomTokenContext {
        mollusk,
        payer: (payer, payer_account.clone()),
        deployer: (deployer, deployer_account.clone()),
        its_root: (its_root_pda, its_root_account.clone()),
        token_mint: (token_mint, token_mint_account.clone()),
    };

    let params = RegisterCustomTokenParams {
        salt,
        token_manager_type,
        operator: None,
    };

    let register_result = execute_register_custom_token_helper(ctx, params, vec![Check::success()]);
    assert!(register_result.result.program_result.is_ok());

    // Now test handover mint authority
    let program_id = solana_axelar_its::id();
    let token_id = {
        let deploy_salt = solana_axelar_its::utils::linked_token_deployer_salt(&deployer, &salt);
        solana_axelar_its::utils::interchain_token_id_internal(&deploy_salt)
    };

    // Get the updated accounts from the register result
    let updated_its_root_account = register_result
        .result
        .get_account(&its_root_pda)
        .unwrap()
        .clone();
    let token_manager_account = register_result
        .result
        .get_account(&register_result.token_manager_pda)
        .unwrap()
        .clone();

    let (minter_roles_pda, _) =
        UserRoles::find_pda(&register_result.token_manager_pda, &mint_authority);

    // Create the handover mint authority instruction
    let instruction_data = solana_axelar_its::instruction::HandoverMintAuthority { token_id };

    let accounts = solana_axelar_its::accounts::HandoverMintAuthority {
        payer,
        authority: mint_authority,
        mint: token_mint,
        its_root: its_root_pda,
        token_manager: register_result.token_manager_pda,
        minter_roles: minter_roles_pda,
        token_program: spl_token_2022::ID,
        system_program: solana_sdk::system_program::ID,
    };

    let ix = Instruction {
        program_id,
        accounts: accounts.to_account_metas(None),
        data: instruction_data.data(),
    };

    // Set up accounts for mollusk
    let mollusk_accounts = vec![
        (payer, payer_account),
        (mint_authority, mint_authority_account),
        (token_mint, token_mint_account),
        (its_root_pda, updated_its_root_account),
        (register_result.token_manager_pda, token_manager_account),
        (minter_roles_pda, new_empty_account()), // empty account since it will be deployed
        mollusk_svm_programs_token::token2022::keyed_account(),
        keyed_account_for_system_program(),
    ];

    let checks = vec![Check::success()];

    let result =
        register_result
            .mollusk
            .process_and_validate_instruction(&ix, &mollusk_accounts, &checks);

    assert!(result.program_result.is_ok());

    // Verify that the mint authority was transferred to the token manager
    let updated_mint_account = result.get_account(&token_mint).unwrap();
    let mint_data = &updated_mint_account.data;
    let mint_state = spl_token_2022::state::Mint::unpack(mint_data).unwrap();

    assert_eq!(
        mint_state.mint_authority,
        Some(register_result.token_manager_pda).into()
    );

    // Verify that the payer received the MINTER role
    let minter_roles_account = result.get_account(&minter_roles_pda).unwrap();
    let user_roles = UserRoles::try_deserialize(&mut minter_roles_account.data.as_ref()).unwrap();

    assert!(user_roles.roles.contains(Roles::MINTER));
}
