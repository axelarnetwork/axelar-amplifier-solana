#![cfg(test)]
#![allow(clippy::too_many_lines)]

use mollusk_svm::Mollusk;
use mollusk_test_utils::setup_mollusk;
use solana_axelar_gateway::seed_prefixes::GATEWAY_SEED;
use solana_axelar_gateway::ID as GATEWAY_PROGRAM_ID;
use solana_axelar_gateway_test_fixtures::{initialize_gateway, setup_test_with_real_signers};
use solana_axelar_its::seed_prefixes::DEPLOYMENT_APPROVAL_SEED;
use solana_axelar_its::state::{TokenManager, UserRoles};
use solana_axelar_its_test_fixtures::{
    approve_deploy_remote_interchain_token_helper, deploy_remote_interchain_token_helper,
    init_gas_service, init_its_service_with_ethereum_trusted, initialize_mollusk, setup_operator,
    ApproveDeployRemoteInterchainTokenContext, DeployRemoteInterchainTokenContext,
};
use solana_axelar_its_test_fixtures::{
    deploy_interchain_token_helper, DeployInterchainTokenContext,
};
use solana_sdk::{account::Account, native_token::LAMPORTS_PER_SOL, pubkey::Pubkey};

#[test]
fn test_deploy_remote_interchain_token_with_minter() {
    let (
        mollusk,
        its_root_pda,
        its_root_account,
        payer,
        payer_account,
        _operator,
        _operator_account,
        treasury_pda,
        gateway_root_pda_account,
    ) = setup_test_environment();

    let program_id = solana_axelar_its::id();

    let deployer = Pubkey::new_unique();
    let deployer_account = Account::new(10 * LAMPORTS_PER_SOL, 0, &solana_sdk::system_program::ID);

    // Create simple token deployment parameters
    let salt = [1u8; 32];
    let name = "Test Token".to_owned();
    let symbol = "TEST".to_owned();
    let decimals = 9u8;
    let initial_supply = 1_000_000_000u64; // 1 billion tokens with 9 decimals

    let minter = Pubkey::new_unique();

    let token_id = solana_axelar_its::utils::interchain_token_id(&deployer, &salt);
    let (token_manager_pda, _) = Pubkey::find_program_address(
        &[
            solana_axelar_its::seed_prefixes::TOKEN_MANAGER_SEED,
            its_root_pda.as_ref(),
            &token_id,
        ],
        &program_id,
    );
    let (minter_roles_pda, _) = Pubkey::find_program_address(
        &[
            solana_axelar_its::state::UserRoles::SEED_PREFIX,
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
        deploy_result,
        token_manager_pda,
        token_mint_pda,
        _token_manager_ata,
        _deployer_ata,
        metadata_account,
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
        deploy_result.program_result.is_ok(),
        "Deploy interchain token instruction should succeed: {:?}",
        deploy_result.program_result
    );

    // Now test approve deploy remote interchain token
    let destination_chain = "ethereum".to_owned();
    let destination_minter = minter.to_bytes().to_vec();

    let destination_chain_hash =
        anchor_lang::solana_program::keccak::hashv(&[destination_chain.as_bytes()]).to_bytes();
    let (deploy_approval_pda, _) = Pubkey::find_program_address(
        &[
            DEPLOYMENT_APPROVAL_SEED,
            minter.as_ref(),
            &token_id,
            &destination_chain_hash,
        ],
        &program_id,
    );

    let ctx = ApproveDeployRemoteInterchainTokenContext::new(
        mollusk,
        deploy_result.clone(),
        minter,
        program_id,
        payer,
        token_manager_pda,
        minter_roles_pda,
        deploy_approval_pda,
    );

    let (approve_result, mollusk) = approve_deploy_remote_interchain_token_helper(
        deployer,
        salt,
        destination_minter.clone(),
        destination_chain.clone(),
        ctx,
    );

    let deploy_approval_pda_account = approve_result.get_account(&deploy_approval_pda).unwrap();
    let minter_roles_pda_account = approve_result.get_account(&minter_roles_pda).unwrap();

    assert!(
        approve_result.program_result.is_ok(),
        "Approve deploy remote interchain token instruction should succeed: {:?}",
        approve_result.program_result
    );

    // Now deploy remote interchain token with remote minter
    let destination_chain = "ethereum".to_owned();
    let gas_value = 0u64;

    let ctx = DeployRemoteInterchainTokenContext::new_with_minter(
        deploy_result,
        mollusk,
        program_id,
        payer,
        deployer,
        token_mint_pda,
        metadata_account,
        token_manager_pda,
        its_root_pda,
        treasury_pda,
        gateway_root_pda_account.clone(),
        minter,
        deploy_approval_pda,
        deploy_approval_pda_account.clone(),
        minter_roles_pda,
        minter_roles_pda_account.clone(),
    );

    let remote_result =
        deploy_remote_interchain_token_helper(salt, destination_chain.clone(), gas_value, ctx);

    assert!(
        remote_result.program_result.is_ok(),
        "Deploy remote interchain token instruction should succeed: {:?}",
        remote_result.program_result
    );
}

#[test]
fn test_deploy_remote_interchain_token_with_wrong_destination_minter() {
    let (
        mollusk,
        its_root_pda,
        its_root_account,
        payer,
        payer_account,
        _operator,
        _operator_account,
        treasury_pda,
        gateway_root_pda_account,
    ) = setup_test_environment();

    let program_id = solana_axelar_its::id();
    let deployer = Pubkey::new_unique();
    let deployer_account = Account::new(10 * LAMPORTS_PER_SOL, 0, &solana_sdk::system_program::ID);

    let salt = [1u8; 32];
    let name = "Test Token".to_owned();
    let symbol = "TEST".to_owned();
    let decimals = 9u8;
    let initial_supply = 1_000_000_000u64;
    let minter = Pubkey::new_unique();

    let token_id = solana_axelar_its::utils::interchain_token_id(&deployer, &salt);
    let (token_manager_pda, _) = TokenManager::find_pda(token_id, its_root_pda);
    let (minter_roles_pda, _) = UserRoles::find_pda(&token_manager_pda, &minter);

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
        deploy_result,
        token_manager_pda,
        token_mint_pda,
        _token_manager_ata,
        _deployer_ata,
        metadata_account,
        mollusk,
    ) = deploy_interchain_token_helper(
        salt,
        name.clone(),
        symbol.clone(),
        decimals,
        initial_supply,
        ctx,
    );

    assert!(deploy_result.program_result.is_ok());

    let destination_chain = "ethereum".to_owned();
    let approved_destination_minter = b"0x1234567890abcdef1234567890abcdef12345678".to_vec();

    let destination_chain_hash =
        anchor_lang::solana_program::keccak::hashv(&[destination_chain.as_bytes()]).to_bytes();
    let (deploy_approval_pda, _) = Pubkey::find_program_address(
        &[
            DEPLOYMENT_APPROVAL_SEED,
            minter.as_ref(),
            &token_id,
            &destination_chain_hash,
        ],
        &program_id,
    );

    let ctx = ApproveDeployRemoteInterchainTokenContext::new(
        mollusk,
        deploy_result.clone(),
        minter,
        program_id,
        payer,
        token_manager_pda,
        minter_roles_pda,
        deploy_approval_pda,
    );

    let (approve_result, mollusk) = approve_deploy_remote_interchain_token_helper(
        deployer,
        salt,
        approved_destination_minter.clone(),
        destination_chain.clone(),
        ctx,
    );

    let deploy_approval_pda_account = approve_result.get_account(&deploy_approval_pda).unwrap();
    let minter_roles_pda_account = approve_result.get_account(&minter_roles_pda).unwrap();

    assert!(approve_result.program_result.is_ok());

    // Now try to deploy with a different destination minter
    let wrong_destination_minter = b"0xdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef".to_vec();
    let gas_value = 0u64;

    let remote_result = deploy_remote_interchain_token_with_custom_minter(
        salt,
        destination_chain.clone(),
        gas_value,
        wrong_destination_minter,
        deploy_result,
        mollusk,
        program_id,
        payer,
        deployer,
        token_mint_pda,
        metadata_account,
        token_manager_pda,
        its_root_pda,
        treasury_pda,
        gateway_root_pda_account.clone(),
        minter,
        deploy_approval_pda,
        deploy_approval_pda_account.clone(),
        minter_roles_pda,
        minter_roles_pda_account.clone(),
    );

    assert!(
        remote_result.program_result.is_err(),
        "Deploy remote interchain token with wrong destination minter should fail: {:?}",
        remote_result.program_result
    );
}

#[allow(clippy::too_many_arguments)]
fn deploy_remote_interchain_token_with_custom_minter(
    salt: [u8; 32],
    destination_chain: String,
    gas_value: u64,
    destination_minter: Vec<u8>,
    result: mollusk_svm::result::InstructionResult,
    mollusk: Mollusk,
    program_id: Pubkey,
    payer: Pubkey,
    deployer: Pubkey,
    token_mint_pda: Pubkey,
    metadata_account: Pubkey,
    token_manager_pda: Pubkey,
    its_root_pda: Pubkey,
    treasury_pda: Account,
    gateway_root_pda_account: Account,
    minter: Pubkey,
    deploy_approval_pda: Pubkey,
    deploy_approval_pda_account: Account,
    minter_roles: Pubkey,
    minter_roles_account: Account,
) -> mollusk_svm::result::InstructionResult {
    use anchor_lang::{InstructionData, ToAccountMetas};
    use mollusk_svm::program::keyed_account_for_system_program;
    use mollusk_test_utils::get_event_authority_and_program_accounts;
    use solana_axelar_gas_service::state::Treasury;
    use solana_axelar_gateway::seed_prefixes::CALL_CONTRACT_SIGNING_SEED;
    use solana_axelar_its::accounts::GasServiceAccounts;
    use solana_sdk::instruction::Instruction;

    let (gateway_root_pda, _) = Pubkey::find_program_address(
        &[solana_axelar_gateway::seed_prefixes::GATEWAY_SEED],
        &solana_axelar_gateway::ID,
    );

    let (gas_treasury, _) =
        Pubkey::find_program_address(&[Treasury::SEED_PREFIX], &solana_axelar_gas_service::ID);

    let (call_contract_signing_pda, _) =
        Pubkey::find_program_address(&[CALL_CONTRACT_SIGNING_SEED], &program_id);

    let (gateway_event_authority, _) =
        Pubkey::find_program_address(&[b"__event_authority"], &solana_axelar_gateway::ID);

    let (gas_event_authority, _) =
        Pubkey::find_program_address(&[b"__event_authority"], &solana_axelar_gas_service::ID);

    let (its_event_authority, its_event_authority_account, its_program_account) =
        get_event_authority_and_program_accounts(&program_id);

    let data = solana_axelar_its::instruction::DeployRemoteInterchainTokenWithMinter {
        salt,
        destination_chain: destination_chain.clone(),
        gas_value,
        destination_minter,
    }
    .data();

    let ix = Instruction {
        program_id,
        accounts: solana_axelar_its::accounts::DeployRemoteInterchainToken {
            payer,
            deployer,
            token_mint: token_mint_pda,
            metadata_account,
            token_manager_pda,
            minter: Some(minter),
            deploy_approval_pda: Some(deploy_approval_pda),
            minter_roles: Some(minter_roles),
            gateway_root_pda,
            gateway_program: solana_axelar_gateway::ID,
            gas_service_accounts: GasServiceAccounts {
                gas_service: solana_axelar_gas_service::ID,
                gas_treasury,
                gas_event_authority,
            },
            system_program: solana_sdk::system_program::ID,
            its_root_pda,
            call_contract_signing_pda,
            gateway_event_authority,
            event_authority: its_event_authority,
            program: program_id,
        }
        .to_account_metas(None),
        data,
    };

    let accounts = vec![
        (payer, result.get_account(&payer).unwrap().clone()),
        (deployer, result.get_account(&deployer).unwrap().clone()),
        (
            token_mint_pda,
            result.get_account(&token_mint_pda).unwrap().clone(),
        ),
        (
            metadata_account,
            result.get_account(&metadata_account).unwrap().clone(),
        ),
        (
            token_manager_pda,
            result.get_account(&token_manager_pda).unwrap().clone(),
        ),
        (
            its_root_pda,
            result.get_account(&its_root_pda).unwrap().clone(),
        ),
        (minter, result.get_account(&minter).unwrap().clone()),
        (minter_roles, minter_roles_account),
        (deploy_approval_pda, deploy_approval_pda_account),
        (gas_treasury, treasury_pda),
        (
            solana_axelar_gas_service::ID,
            Account {
                lamports: 1,
                data: vec![],
                owner: solana_sdk::bpf_loader_upgradeable::id(),
                executable: true,
                rent_epoch: 0,
            },
        ),
        (
            gas_event_authority,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
        (gateway_root_pda, gateway_root_pda_account),
        (
            call_contract_signing_pda,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
        (
            solana_axelar_gateway::ID,
            Account {
                lamports: 1,
                data: vec![],
                owner: solana_sdk::bpf_loader_upgradeable::id(),
                executable: true,
                rent_epoch: 0,
            },
        ),
        (
            gateway_event_authority,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
        keyed_account_for_system_program(),
        (its_event_authority, its_event_authority_account),
        (program_id, its_program_account),
    ];

    mollusk.process_instruction(&ix, &accounts)
}

// temporary setup function for tests
// TODO fix/refactor the setup
fn setup_test_environment() -> (
    Mollusk,
    Pubkey,
    Account,
    Pubkey,
    Account,
    Pubkey,
    Account,
    Account,
    Account,
) {
    let (setup, _, _, _, _) = setup_test_with_real_signers();
    let init_result = initialize_gateway(&setup);
    assert!(init_result.program_result.is_ok());

    let gas_service_program_id = solana_axelar_gas_service::id();
    let mut mollusk = setup_mollusk(&gas_service_program_id, "solana_axelar_gas_service");

    let operator = Pubkey::new_unique();
    let operator_account = Account::new(1_000_000_000, 0, &solana_sdk::system_program::ID);

    let (operator_pda, operator_pda_account) =
        setup_operator(&mut mollusk, operator, &operator_account);

    let (_, treasury_pda) = init_gas_service(
        &mollusk,
        operator,
        &operator_account,
        operator_pda,
        &operator_pda_account,
    );

    let (gateway_root_pda, _) = Pubkey::find_program_address(&[GATEWAY_SEED], &GATEWAY_PROGRAM_ID);
    let gateway_root_pda_account = init_result.get_account(&gateway_root_pda).unwrap().clone();

    let mollusk = initialize_mollusk();

    let payer = Pubkey::new_unique();
    let payer_account = Account::new(10 * LAMPORTS_PER_SOL, 0, &solana_sdk::system_program::ID);

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

    (
        mollusk,
        its_root_pda,
        its_root_account,
        payer,
        payer_account,
        operator,
        operator_account,
        treasury_pda,
        gateway_root_pda_account,
    )
}
