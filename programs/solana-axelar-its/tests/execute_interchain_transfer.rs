#![cfg(test)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::indexing_slicing)]

use anchor_lang::{AccountDeserialize, InstructionData, ToAccountMetas};
use anchor_spl::{
    associated_token::spl_associated_token_account,
    token_2022::spl_token_2022::{self},
};
use interchain_token_transfer_gmp::{
    DeployInterchainToken, GMPPayload, InterchainTransfer, ReceiveFromHub,
};
use mollusk_svm::program::keyed_account_for_system_program;
use mollusk_test_utils::get_event_authority_and_program_accounts;
use solana_axelar_gateway::{GatewayConfig, ID as GATEWAY_PROGRAM_ID};
use solana_axelar_gateway_test_fixtures::{
    approve_messages_on_gateway, create_test_message, initialize_gateway,
    setup_test_with_real_signers,
};
use solana_axelar_its::{state::TokenManager, utils::interchain_token_id};
use solana_axelar_its_test_fixtures::{
    create_rent_sysvar_data, create_sysvar_instructions_data,
    init_its_service_with_ethereum_trusted, initialize_mollusk,
};
use solana_program::program_pack::{IsInitialized, Pack};
use solana_sdk::{
    account::Account, instruction::Instruction, keccak, native_token::LAMPORTS_PER_SOL,
    pubkey::Pubkey,
};
use spl_token_2022::{extension::StateWithExtensions, state::Account as Token2022Account};

#[test]
fn test_execute_interchain_transfer_success() {
    // Step 1: Setup gateway with real signers
    let (mut setup, verifier_leaves, verifier_merkle_tree, secret_key_1, secret_key_2) =
        setup_test_with_real_signers();

    // Step 2: Initialize gateway
    let init_result = initialize_gateway(&setup);
    let (gateway_root_pda, _) = GatewayConfig::find_pda();
    let gateway_root_pda_account = init_result.get_account(&gateway_root_pda).unwrap();

    assert!(init_result.program_result.is_ok());

    // Step 3: Add ITS program to mollusk
    let program_id = solana_axelar_its::id();
    let mut mollusk = initialize_mollusk();
    mollusk.add_program(
        &GATEWAY_PROGRAM_ID,
        "../../target/deploy/solana_axelar_gateway",
        &solana_sdk::bpf_loader_upgradeable::id(),
    );
    setup.mollusk = mollusk;

    // Step 4: Initialize ITS service
    let payer = Pubkey::new_unique();
    let payer_account = Account::new(10 * LAMPORTS_PER_SOL, 0, &solana_sdk::system_program::ID);

    let operator = Pubkey::new_unique();
    let operator_account = Account::new(1_000_000_000, 0, &solana_sdk::system_program::ID);

    let chain_name = "solana".to_owned();
    let its_hub_address = "0x123456789abcdef".to_owned();

    let (its_root_pda, its_root_account) = init_its_service_with_ethereum_trusted(
        &setup.mollusk,
        payer,
        &payer_account,
        payer,
        operator,
        &operator_account,
        chain_name.clone(),
        its_hub_address.clone(),
    );

    // Step 5: First deploy a token so we have a token manager with mint authority
    let salt = [1u8; 32];
    let token_id = interchain_token_id(&payer, &salt);
    let name = "Test Token".to_owned();
    let symbol = "TEST".to_owned();
    let decimals = 9u8;

    // Create the deploy token GMP payload
    let deploy_payload = DeployInterchainToken {
        selector: alloy_primitives::U256::from(1), // MESSAGE_TYPE_ID for DeployInterchainToken
        token_id: alloy_primitives::FixedBytes::from(token_id),
        name: name.clone(),
        symbol: symbol.clone(),
        decimals,
        minter: alloy_primitives::Bytes::new(), // Empty bytes for no minter
    };

    // Wrap in ReceiveFromHub payload
    let deploy_receive_from_hub_payload = ReceiveFromHub {
        selector: alloy_primitives::U256::from(4), // MESSAGE_TYPE_ID for ReceiveFromHub
        source_chain: "ethereum".to_owned(),
        payload: GMPPayload::DeployInterchainToken(deploy_payload)
            .encode()
            .into(),
    };

    let deploy_gmp_payload = GMPPayload::ReceiveFromHub(deploy_receive_from_hub_payload);
    let deploy_encoded_payload = deploy_gmp_payload.encode();
    let deploy_payload_hash = keccak::hashv(&[&deploy_encoded_payload]).to_bytes();

    // Create test message for deploy
    let mut deploy_message = create_test_message(
        "ethereum",
        "deploy_token_123",
        &program_id.to_string(),
        deploy_payload_hash,
    );

    // Override the source_address to match its_hub_address
    deploy_message.source_address = its_hub_address.clone();

    // Approve deploy message on gateway
    let deploy_incoming_messages = approve_messages_on_gateway(
        &setup,
        vec![deploy_message.clone()],
        init_result.clone(),
        &secret_key_1,
        &secret_key_2,
        verifier_leaves.clone(),
        verifier_merkle_tree.clone(),
    );

    let (_, deploy_incoming_message_pda, deploy_incoming_message_account_data) =
        &deploy_incoming_messages[0];

    // Find required PDAs
    let (token_manager_pda, _) = Pubkey::find_program_address(
        &[
            solana_axelar_its::seed_prefixes::TOKEN_MANAGER_SEED,
            its_root_pda.as_ref(),
            &token_id,
        ],
        &program_id,
    );

    let (token_mint_pda, _) = Pubkey::find_program_address(
        &[
            solana_axelar_its::seed_prefixes::INTERCHAIN_TOKEN_SEED,
            its_root_pda.as_ref(),
            &token_id,
        ],
        &program_id,
    );

    let (token_manager_ata, _) = Pubkey::find_program_address(
        &[
            token_manager_pda.as_ref(),
            spl_token_2022::id().as_ref(),
            token_mint_pda.as_ref(),
        ],
        &spl_associated_token_account::id(),
    );

    let (deployer_ata, _) = Pubkey::find_program_address(
        &[
            payer.as_ref(),
            spl_token_2022::id().as_ref(),
            token_mint_pda.as_ref(),
        ],
        &spl_associated_token_account::id(),
    );

    let (metadata_account, _) = Pubkey::find_program_address(
        &[
            b"metadata",
            mpl_token_metadata::ID.as_ref(),
            token_mint_pda.as_ref(),
        ],
        &mpl_token_metadata::ID,
    );

    let (deploy_signing_pda, _) = Pubkey::find_program_address(
        &[
            solana_axelar_gateway::seed_prefixes::VALIDATE_MESSAGE_SIGNING_SEED,
            deploy_message.command_id().as_ref(),
        ],
        &program_id,
    );

    let (gateway_event_authority, _, _) =
        get_event_authority_and_program_accounts(&solana_axelar_gateway::ID);

    let (its_event_authority, event_authority_account, its_program_account) =
        get_event_authority_and_program_accounts(&program_id);

    // Execute deploy instruction first
    let deploy_instruction_data = solana_axelar_its::instruction::Execute {
        message: deploy_message.clone(),
        payload: deploy_encoded_payload,
    };

    let deploy_executable_accounts = solana_axelar_its::accounts::AxelarExecuteAccounts {
        incoming_message_pda: *deploy_incoming_message_pda,
        signing_pda: deploy_signing_pda,
        gateway_root_pda,
        event_authority: gateway_event_authority,
        axelar_gateway_program: GATEWAY_PROGRAM_ID,
    };

    let deploy_accounts = solana_axelar_its::accounts::Execute {
        // GMP accounts
        executable: deploy_executable_accounts,

        // ITS accounts
        payer,
        its_root_pda,
        token_manager_pda,
        token_mint: token_mint_pda,
        token_manager_ata,
        token_program: spl_token_2022::id(),
        associated_token_program: spl_associated_token_account::id(),
        system_program: solana_sdk::system_program::ID,

        // Event CPI accounts
        event_authority: its_event_authority,
        program: program_id,
    };

    let mut deploy_account_metas = deploy_accounts.to_account_metas(None);
    deploy_account_metas.extend(
        solana_axelar_its::instructions::gmp::execute::execute_deploy_interchain_token_extra_accounts(
            deployer_ata,
            payer,
            solana_sdk::sysvar::instructions::ID,
            mpl_token_metadata::ID,
            metadata_account,
            None,
            None,
        ),
    );

    let deploy_execute_instruction = Instruction {
        program_id,
        accounts: deploy_account_metas,
        data: deploy_instruction_data.data(),
    };

    let deploy_incoming_message_account = Account {
        lamports: setup
            .mollusk
            .sysvars
            .rent
            .minimum_balance(deploy_incoming_message_account_data.len()),
        data: deploy_incoming_message_account_data.clone(),
        owner: GATEWAY_PROGRAM_ID,
        executable: false,
        rent_epoch: 0,
    };

    let deploy_execute_accounts = vec![
        // AxelarExecuteAccounts
        (
            *deploy_incoming_message_pda,
            deploy_incoming_message_account,
        ),
        (
            deploy_signing_pda,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
        (gateway_root_pda, gateway_root_pda_account.clone()),
        (
            gateway_event_authority,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
        (
            GATEWAY_PROGRAM_ID,
            Account {
                lamports: LAMPORTS_PER_SOL,
                data: vec![],
                owner: solana_sdk::bpf_loader_upgradeable::id(),
                executable: true,
                rent_epoch: 0,
            },
        ),
        // ITS Accounts
        (payer, payer_account.clone()),
        (its_root_pda, its_root_account.clone()),
        (
            token_manager_pda,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
        (
            token_mint_pda,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
        (
            token_manager_ata,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
        mollusk_svm_programs_token::token2022::keyed_account(),
        mollusk_svm_programs_token::associated_token::keyed_account(),
        (
            solana_sdk::sysvar::rent::ID,
            Account {
                lamports: 1_000_000_000,
                data: create_rent_sysvar_data(),
                owner: solana_sdk::sysvar::rent::ID,
                executable: false,
                rent_epoch: 0,
            },
        ),
        keyed_account_for_system_program(),
        // Extra accounts for DeployInterchainToken
        (
            deployer_ata,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
        (payer, payer_account.clone()),
        (
            solana_sdk::sysvar::instructions::ID,
            Account {
                lamports: 1_000_000_000,
                data: create_sysvar_instructions_data(),
                owner: solana_program::sysvar::id(),
                executable: false,
                rent_epoch: 0,
            },
        ),
        (
            mpl_token_metadata::ID,
            Account {
                lamports: 1,
                data: vec![],
                owner: solana_sdk::bpf_loader_upgradeable::id(),
                executable: true,
                rent_epoch: 0,
            },
        ),
        (
            metadata_account,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
        // Event CPI accounts
        (its_event_authority, event_authority_account.clone()),
        (program_id, its_program_account.clone()),
    ];

    // Execute the deploy instruction
    let deploy_result = setup
        .mollusk
        .process_instruction(&deploy_execute_instruction, &deploy_execute_accounts);

    // Verify deploy success
    assert!(
        deploy_result.program_result.is_ok(),
        "Deploy instruction should succeed: {:?}",
        deploy_result.program_result
    );

    // Step 6: Now create the interchain transfer using the deployed token
    let transfer_amount = 1_000_000u64;
    let destination_pubkey = payer; // Transfer to same user for simplicity
    let destination_address = destination_pubkey.to_bytes().to_vec(); // 32 bytes for Solana Pubkey
    let source_address = "ethereum_address_123".to_owned(); // Valid UTF-8 string

    let source_address_bytes = source_address.as_bytes().to_vec();

    let transfer_payload = InterchainTransfer {
        selector: alloy_primitives::U256::from(0), // MESSAGE_TYPE_ID for InterchainTransfer
        token_id: alloy_primitives::FixedBytes::from(token_id),
        source_address: alloy_primitives::Bytes::from(source_address_bytes.clone()),
        destination_address: alloy_primitives::Bytes::from(destination_address.clone()),
        amount: alloy_primitives::U256::from(transfer_amount),
        data: alloy_primitives::Bytes::new(), // Empty data for simple transfer
    };

    let transfer_receive_from_hub_payload = ReceiveFromHub {
        selector: alloy_primitives::U256::from(4),
        source_chain: "ethereum".to_owned(),
        payload: GMPPayload::InterchainTransfer(transfer_payload)
            .encode()
            .into(),
    };

    let transfer_gmp_payload = GMPPayload::ReceiveFromHub(transfer_receive_from_hub_payload);
    let transfer_encoded_payload = transfer_gmp_payload.encode();
    let transfer_payload_hash = keccak::hashv(&[&transfer_encoded_payload]).to_bytes();

    let mut transfer_message = create_test_message(
        "ethereum",
        "transfer_token_456",
        &program_id.to_string(),
        transfer_payload_hash,
    );
    transfer_message.source_address = its_hub_address;

    let transfer_incoming_messages = approve_messages_on_gateway(
        &setup,
        vec![transfer_message.clone()],
        init_result.clone(),
        &secret_key_1,
        &secret_key_2,
        verifier_leaves,
        verifier_merkle_tree,
    );

    let (_, transfer_incoming_message_pda, transfer_incoming_message_account_data) =
        &transfer_incoming_messages[0];

    let (transfer_signing_pda, _) = Pubkey::find_program_address(
        &[
            solana_axelar_gateway::seed_prefixes::VALIDATE_MESSAGE_SIGNING_SEED,
            transfer_message.command_id().as_ref(),
        ],
        &program_id,
    );

    // Create destination ATA
    let (destination_ata, _) = Pubkey::find_program_address(
        &[
            destination_pubkey.as_ref(),
            spl_token_2022::id().as_ref(),
            token_mint_pda.as_ref(),
        ],
        &spl_associated_token_account::id(),
    );

    // Execute transfer instruction
    let transfer_instruction_data = solana_axelar_its::instruction::Execute {
        message: transfer_message.clone(),
        payload: transfer_encoded_payload,
    };

    let transfer_executable_accounts = solana_axelar_its::accounts::AxelarExecuteAccounts {
        incoming_message_pda: *transfer_incoming_message_pda,
        signing_pda: transfer_signing_pda,
        gateway_root_pda,
        event_authority: gateway_event_authority,
        axelar_gateway_program: GATEWAY_PROGRAM_ID,
    };

    let transfer_accounts = solana_axelar_its::accounts::Execute {
        executable: transfer_executable_accounts,
        payer,
        its_root_pda,
        token_manager_pda,
        token_mint: token_mint_pda,
        token_manager_ata,
        token_program: spl_token_2022::id(),
        associated_token_program: spl_associated_token_account::id(),
        system_program: solana_sdk::system_program::ID,
        event_authority: its_event_authority,
        program: program_id,
    };

    let mut transfer_account_metas = transfer_accounts.to_account_metas(None);
    transfer_account_metas.extend(
        solana_axelar_its::instructions::gmp::execute::execute_interchain_transfer_extra_accounts(
            destination_pubkey,
            destination_ata,
        ),
    );

    let transfer_execute_instruction = Instruction {
        program_id,
        accounts: transfer_account_metas,
        data: transfer_instruction_data.data(),
    };

    let transfer_incoming_message_account = Account {
        lamports: setup
            .mollusk
            .sysvars
            .rent
            .minimum_balance(transfer_incoming_message_account_data.len()),
        data: transfer_incoming_message_account_data.clone(),
        owner: GATEWAY_PROGRAM_ID,
        executable: false,
        rent_epoch: 0,
    };

    // Get updated accounts from deploy result
    let token_manager_account_after_deploy = deploy_result.get_account(&token_manager_pda).unwrap();
    let token_mint_account_after_deploy = deploy_result.get_account(&token_mint_pda).unwrap();
    let token_manager_ata_after_deploy = deploy_result.get_account(&token_manager_ata).unwrap();
    let deployer_ata_account_after_deploy = deploy_result.get_account(&deployer_ata).unwrap();

    let transfer_execute_accounts = vec![
        (
            *transfer_incoming_message_pda,
            transfer_incoming_message_account,
        ),
        (
            transfer_signing_pda,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
        (gateway_root_pda, gateway_root_pda_account.clone()),
        (
            gateway_event_authority,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
        (
            GATEWAY_PROGRAM_ID,
            Account {
                lamports: LAMPORTS_PER_SOL,
                data: vec![],
                owner: solana_sdk::bpf_loader_upgradeable::id(),
                executable: true,
                rent_epoch: 0,
            },
        ),
        (payer, payer_account.clone()),
        (its_root_pda, its_root_account),
        (
            token_manager_pda,
            token_manager_account_after_deploy.clone(),
        ),
        (token_mint_pda, token_mint_account_after_deploy.clone()),
        (token_manager_ata, token_manager_ata_after_deploy.clone()),
        mollusk_svm_programs_token::token2022::keyed_account(),
        mollusk_svm_programs_token::associated_token::keyed_account(),
        (
            solana_sdk::sysvar::rent::ID,
            Account {
                lamports: 1_000_000_000,
                data: create_rent_sysvar_data(),
                owner: solana_sdk::sysvar::rent::ID,
                executable: false,
                rent_epoch: 0,
            },
        ),
        keyed_account_for_system_program(),
        // Extra accounts for InterchainTransfer
        (destination_pubkey, payer_account.clone()),
        (destination_ata, deployer_ata_account_after_deploy.clone()),
        // Event CPI accounts
        (its_event_authority, event_authority_account),
        (program_id, its_program_account),
    ];

    let transfer_result = setup
        .mollusk
        .process_instruction(&transfer_execute_instruction, &transfer_execute_accounts);

    // Verify transfer success
    assert!(
        transfer_result.program_result.is_ok(),
        "Transfer instruction should succeed: {:?}",
        transfer_result.program_result
    );

    // Verify token manager exists and is correct
    let token_manager_account = transfer_result.get_account(&token_manager_pda).unwrap();
    let token_manager =
        TokenManager::try_deserialize(&mut token_manager_account.data.as_ref()).unwrap();
    assert_eq!(token_manager.token_id, token_id);

    // Verify that the destination got their tokens
    let destination_ata_account_after = transfer_result.get_account(&destination_ata).unwrap();
    let destination_ata_data =
        StateWithExtensions::<Token2022Account>::unpack(&destination_ata_account_after.data)
            .unwrap();

    assert_eq!(destination_ata_data.base.amount, transfer_amount,);
    assert_eq!(destination_ata_data.base.mint, token_mint_pda,);
    assert_eq!(destination_ata_data.base.owner, destination_pubkey,);
    assert!(destination_ata_data.base.is_initialized());

    let token_mint_account_after = transfer_result.get_account(&token_mint_pda).unwrap();
    let token_mint_after =
        spl_token_2022::state::Mint::unpack(&token_mint_account_after.data).unwrap();

    assert_eq!(token_mint_after.supply, transfer_amount,);
}
