use anchor_lang::AccountDeserialize;
use anchor_lang::InstructionData;
use anchor_lang::ToAccountMetas;
use anchor_spl::{
    token::spl_token,
    token_2022::spl_token_2022::{self},
};
use axelar_solana_its_v2::{
    seed_prefixes::DEPLOYMENT_APPROVAL_SEED, state::deploy_approval::DeployApproval,
};
use axelar_solana_its_v2_test_fixtures::{
    deploy_interchain_token_helper, DeployInterchainTokenContext,
};
use mollusk_svm_programs_token;
use mollusk_test_utils::{get_event_authority_and_program_accounts, setup_mollusk};
use solana_sdk::{
    account::Account, native_token::LAMPORTS_PER_SOL, pubkey::Pubkey, system_program,
};

use spl_token_metadata_interface::solana_instruction::Instruction;

#[path = "initialize.rs"]
mod initialize;

#[test]
fn test_approve_deploy_remote_interchain_token() {
    let program_id = axelar_solana_its_v2::id();
    let mut mollusk = setup_mollusk(&program_id, "axelar_solana_its_v2");

    mollusk.add_program(
        &mpl_token_metadata::programs::MPL_TOKEN_METADATA_ID,
        "../../target/deploy/mpl_token_metadata",
        &solana_sdk::bpf_loader_upgradeable::id(),
    );

    let spl_token_elf = mollusk_svm_programs_token::token::ELF;
    mollusk.add_program_with_elf_and_loader(
        &spl_token::ID,
        &spl_token_elf,
        &solana_sdk::bpf_loader_upgradeable::ID,
    );

    let token_2022_elf = mollusk_svm_programs_token::token2022::ELF;
    mollusk.add_program_with_elf_and_loader(
        &spl_token_2022::ID,
        &token_2022_elf,
        &solana_sdk::bpf_loader_upgradeable::ID,
    );

    let associated_token_elf = mollusk_svm_programs_token::associated_token::ELF;
    mollusk.add_program_with_elf_and_loader(
        &anchor_spl::associated_token::ID,
        &associated_token_elf,
        &solana_sdk::bpf_loader_upgradeable::ID,
    );

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
    let (minter_roles_pda, _) = Pubkey::find_program_address(
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

    let (result, token_manager_pda, _, _, _, _, mollusk) = deploy_interchain_token_helper(
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

    // Now test approve deploy remote interchain token
    let destination_chain = "ethereum".to_string();
    let destination_minter = b"0x1234567890abcdef1234567890abcdef12345678".to_vec();

    let destination_chain_hash =
        anchor_lang::solana_program::keccak::hashv(&[destination_chain.as_bytes()]).to_bytes();
    let (deploy_approval_pda, deploy_approval_bump) = Pubkey::find_program_address(
        &[
            DEPLOYMENT_APPROVAL_SEED,
            minter.as_ref(),
            &token_id,
            &destination_chain_hash,
        ],
        &program_id,
    );

    let (event_authority, event_authority_account, program_account) =
        get_event_authority_and_program_accounts(&program_id);

    let approve_ix_data = axelar_solana_its_v2::instruction::ApproveDeployRemoteInterchainToken {
        deployer,
        salt,
        destination_chain: destination_chain.clone(),
        destination_minter: destination_minter.clone(),
    }
    .data();

    let approve_ix = Instruction {
        program_id,
        accounts: axelar_solana_its_v2::accounts::ApproveDeployRemoteInterchainToken {
            payer,
            minter,
            token_manager_pda,
            minter_roles: minter_roles_pda,
            deploy_approval_pda,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
        data: approve_ix_data,
    };

    let updated_payer_account = result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == payer)
        .map(|(_, account)| account.clone())
        .unwrap_or_else(|| Account::new(9 * LAMPORTS_PER_SOL, 0, &system_program::ID));

    let updated_token_manager_account = result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == token_manager_pda)
        .unwrap()
        .1
        .clone();

    let updated_minter_roles_account = result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == minter_roles_pda)
        .unwrap()
        .1
        .clone();

    let approve_accounts = vec![
        (payer, updated_payer_account),
        (minter, Account::new(0, 0, &system_program::ID)),
        (token_manager_pda, updated_token_manager_account),
        (minter_roles_pda, updated_minter_roles_account),
        (deploy_approval_pda, Account::new(0, 0, &system_program::ID)),
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
        // For event CPI
        (event_authority, event_authority_account),
        (program_id, program_account),
    ];

    let approve_result = mollusk.process_instruction(&approve_ix, &approve_accounts);

    assert!(
        approve_result.program_result.is_ok(),
        "Approve deploy remote interchain token instruction should succeed: {:?}",
        approve_result.program_result
    );

    let deploy_approval_account = approve_result.get_account(&deploy_approval_pda).unwrap();
    let deploy_approval =
        DeployApproval::try_deserialize(&mut deploy_approval_account.data.as_ref()).unwrap();

    let expected_destination_minter_hash =
        anchor_lang::solana_program::keccak::hash(&destination_minter).to_bytes();
    assert_eq!(
        deploy_approval.approved_destination_minter, expected_destination_minter_hash,
        "Approved destination minter hash should match"
    );
    assert_eq!(
        deploy_approval.bump, deploy_approval_bump,
        "Deploy approval bump should match"
    );
}
