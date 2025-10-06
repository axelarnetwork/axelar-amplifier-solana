use alloy_sol_types::SolValue;
use anchor_lang::prelude::AccountMeta;
use anchor_lang::AnchorSerialize;
use anchor_lang::{solana_program, AccountDeserialize};
use axelar_solana_gateway_v2::IncomingMessage;
use axelar_solana_gateway_v2_test_fixtures::{
    approve_messages_on_gateway, create_test_message, initialize_gateway,
    setup_test_with_real_signers,
};
use axelar_solana_governance::seed_prefixes;
use axelar_solana_governance_v2::ExecutableProposal;
use axelar_solana_governance_v2::SolanaAccountMetadata;
use axelar_solana_governance_v2::ID as GOVERNANCE_PROGRAM_ID;
use axelar_solana_governance_v2::{state::GovernanceConfig, GovernanceConfigUpdate};
use axelar_solana_governance_v2_test_fixtures::{
    create_execute_operator_proposal_instruction_data, create_execute_proposal_instruction_data,
    create_gateway_event_authority_pda, create_governance_config_pda,
    create_governance_event_authority_pda, create_governance_program_data_pda,
    create_operator_proposal_pda, create_proposal_pda, create_signing_pda_from_message,
    create_transfer_operatorship_instruction_data, extract_proposal_hash_unchecked,
    get_memo_instruction_data, get_withdraw_tokens_instruction_data, initialize_governance,
    mock_setup_test, process_gmp_helper, update_config, GmpContext, TestSetup,
};
use axelar_solana_memo_v2::ID as MEMO_PROGRAM_ID;
use governance_gmp::alloy_primitives::U256;
use hex::FromHex;
use solana_sdk::account::Account;
use solana_sdk::clock::Clock;
use solana_sdk::instruction::Instruction;
use solana_sdk::native_token::LAMPORTS_PER_SOL;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::system_program::ID as SYSTEM_PROGRAM_ID;

#[test]
fn should_initialize_config() {
    let setup = mock_setup_test();
    let chain_hash = [1u8; 32];
    let address_hash = [2u8; 32];
    let minimum_proposal_eta_delay = 3600;

    let governance_config = GovernanceConfig::new(
        chain_hash,
        address_hash,
        minimum_proposal_eta_delay,
        setup.operator.to_bytes(),
    );

    let result = initialize_governance(&setup, governance_config.clone());
    assert!(!result.program_result.is_err());

    let governance_config_account = result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == setup.governance_config)
        .unwrap()
        .1
        .clone();

    let actual_config =
        GovernanceConfig::try_deserialize(&mut governance_config_account.data.as_slice()).unwrap();

    assert_eq!(actual_config.chain_hash, governance_config.chain_hash);
    assert_eq!(
        actual_config.minimum_proposal_eta_delay,
        governance_config.minimum_proposal_eta_delay
    );
    assert_eq!(actual_config.operator, governance_config.operator);
}

#[test]
fn should_update_config() {
    let setup = mock_setup_test();
    let chain_hash = [1u8; 32];
    let address_hash = [2u8; 32];
    let minimum_proposal_eta_delay = 3600;

    let governance_config = GovernanceConfig::new(
        chain_hash,
        address_hash,
        minimum_proposal_eta_delay,
        setup.operator.to_bytes(),
    );

    let result = initialize_governance(&setup, governance_config.clone());
    assert!(!result.program_result.is_err());

    let governance_config_account = result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == setup.governance_config)
        .unwrap()
        .1
        .clone();

    let new_chain_hash = [3u8; 32];
    let new_address_hash = [4u8; 32];
    let new_minimum_proposal_eta_delay = 7200;

    let params = GovernanceConfigUpdate {
        chain_hash: Some(new_chain_hash),
        address_hash: Some(new_address_hash),
        minimum_proposal_eta_delay: Some(new_minimum_proposal_eta_delay),
    };

    let result = update_config(&setup, params.clone(), governance_config_account.data);
    assert!(!result.program_result.is_err());

    let governance_config_account = result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == setup.governance_config)
        .unwrap()
        .1
        .clone();

    let updated_config =
        GovernanceConfig::try_deserialize(&mut governance_config_account.data.as_slice()).unwrap();

    assert_eq!(updated_config.chain_hash, new_chain_hash);
}

#[test]
fn should_schedule_timelock_proposal() {
    // Step 1: Setup gateway with real signers
    let (mut setup, verifier_leaves, verifier_merkle_tree, secret_key_1, secret_key_2) =
        setup_test_with_real_signers();

    // Step 2: Initialize gateway
    let init_result = initialize_gateway(&setup);
    assert!(!init_result.program_result.is_err());

    let schedule_payload: Vec<u8> = Vec::from_hex("0000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000a000000000000000000000000000000000000000000000000000000000000000e000000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000068d40fa100000000000000000000000000000000000000000000000000000000000000208e3ada0bc9a65c73374363655898f17ad104ea9822d37be8d954e72b2dcb0a36000000000000000000000000000000000000000000000000000000000000004e010000000000000000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000").unwrap();
    let schedule_payload_hash = solana_program::keccak::hashv(&[&schedule_payload]).to_bytes();

    let cancel_payload: Vec<u8> = Vec::from_hex("0000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000a000000000000000000000000000000000000000000000000000000000000000e000000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000068d69bdc00000000000000000000000000000000000000000000000000000000000000208e3ada0bc9a65c73374363655898f17ad104ea9822d37be8d954e72b2dcb0a36000000000000000000000000000000000000000000000000000000000000004e010000000000000000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000").unwrap();
    let cancel_payload_hash = solana_program::keccak::hashv(&[&cancel_payload]).to_bytes();

    let messages = vec![
        create_test_message(
            "ethereum",
            "msg_id_1",
            &GOVERNANCE_PROGRAM_ID.to_string(),
            schedule_payload_hash,
        ),
        create_test_message(
            "ethereum",
            "msg_id_2",
            &GOVERNANCE_PROGRAM_ID.to_string(),
            cancel_payload_hash,
        ),
    ];

    let incoming_messages = approve_messages_on_gateway(
        &setup,
        messages.clone(),
        init_result,
        &secret_key_1,
        &secret_key_2,
        verifier_leaves,
        verifier_merkle_tree,
    );

    // Now we have an approved message
    // Setup Governance
    setup.mollusk.add_program(
        &GOVERNANCE_PROGRAM_ID,
        "../../target/deploy/axelar_solana_governance_v2",
        &solana_sdk::bpf_loader_upgradeable::id(),
    );

    let payer = Pubkey::new_unique();
    let upgrade_authority = Pubkey::new_unique();
    let operator = Pubkey::new_unique();

    let (governance_config, governance_config_bump) = create_governance_config_pda();
    let program_data_pda = create_governance_program_data_pda();
    let (event_authority_pda_governance, event_authority_bump) =
        create_governance_event_authority_pda();

    let chain_hash = solana_program::keccak::hashv(&[b"ethereum"]).to_bytes();
    let address_hash =
        solana_program::keccak::hashv(&["0xSourceAddress".to_string().as_bytes()]).to_bytes();
    let minimum_proposal_eta_delay = 3600;

    let governance_setup = TestSetup {
        mollusk: setup.mollusk,
        payer,
        upgrade_authority,
        operator,
        governance_config,
        governance_config_bump,
        program_data_pda,
        event_authority_pda: event_authority_pda_governance,
        event_authority_bump,
    };

    let governance_config = GovernanceConfig::new(
        chain_hash,
        address_hash,
        minimum_proposal_eta_delay,
        operator.to_bytes(),
    );

    let result = initialize_governance(&governance_setup, governance_config.clone());
    assert!(!result.program_result.is_err());

    let governance_config_account = result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == governance_setup.governance_config)
        .unwrap()
        .1
        .clone();

    let message = messages[0].clone();
    let schedule_incoming_message_account = incoming_messages[0].clone().0;
    let schedule_incoming_message_pda = incoming_messages[0].clone().1;
    let schedule_incoming_message_account_data = incoming_messages[0].clone().2;

    let signing_pda = create_signing_pda_from_message(&message, &schedule_incoming_message_account);
    let event_authority_pda_gateway = create_gateway_event_authority_pda();
    let (event_authority_pda_governance, _) = create_governance_event_authority_pda();
    let proposal_hash = extract_proposal_hash_unchecked(&schedule_payload);
    let proposal_pda = create_proposal_pda(&proposal_hash);

    let gmp_context = GmpContext::new()
        .with_incoming_message(
            schedule_incoming_message_pda,
            schedule_incoming_message_account_data.clone(),
        )
        .with_governance_config(
            governance_setup.governance_config,
            governance_config_account.data.clone(),
        )
        .with_signing_pda(signing_pda)
        .with_event_authority_pda(event_authority_pda_gateway)
        .with_event_authority_pda_governance(event_authority_pda_governance)
        .with_proposal(proposal_pda, vec![], SYSTEM_PROGRAM_ID);

    // Send schedule timelock proposal
    let result = process_gmp_helper(
        &governance_setup,
        messages[0].clone(),
        schedule_payload,
        gmp_context,
    );
    assert!(!result.program_result.is_err());

    let proposal_pda_account = result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == proposal_pda)
        .unwrap()
        .1
        .clone();

    let _ = ExecutableProposal::try_deserialize(&mut proposal_pda_account.data.as_slice()).unwrap();

    let cancel_message = messages[1].clone();
    let cancel_incoming_message = incoming_messages[1].clone().0;
    let cancel_incoming_message_pda = incoming_messages[1].1;
    let cancel_incoming_message_account_data = incoming_messages[1].clone().2;

    let cancel_signing_pda =
        create_signing_pda_from_message(&cancel_message, &cancel_incoming_message);

    let gmp_context = GmpContext::new()
        .with_incoming_message(
            cancel_incoming_message_pda,
            cancel_incoming_message_account_data,
        )
        .with_governance_config(
            governance_setup.governance_config,
            governance_config_account.data,
        )
        .with_signing_pda(cancel_signing_pda)
        .with_event_authority_pda(event_authority_pda_gateway)
        .with_event_authority_pda_governance(event_authority_pda_governance)
        .with_proposal(
            proposal_pda,
            proposal_pda_account.data,
            GOVERNANCE_PROGRAM_ID,
        );

    // Send cancel timelock proposal
    let result = process_gmp_helper(
        &governance_setup,
        messages[1].clone(),
        cancel_payload,
        gmp_context,
    );
    assert!(!result.program_result.is_err());

    let proposal_pda_account = result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == proposal_pda)
        .unwrap()
        .1
        .clone();

    assert_eq!(proposal_pda_account.data.len(), 0);
}

#[test]
fn should_full_governance_workflow_schedule_and_approve_operator() {
    // Step 1: Setup gateway with real signers
    let (mut setup, verifier_leaves, verifier_merkle_tree, secret_key_1, secret_key_2) =
        setup_test_with_real_signers();

    // Step 2: Initialize gateway
    let init_result = initialize_gateway(&setup);
    assert!(!init_result.program_result.is_err());

    // Step 3: Create ALL 4 governance message payloads
    // These payloads represent the same proposal but different commands

    // Schedule timelock proposal (command = 0)
    let schedule_payload: Vec<u8> = Vec::from_hex("0000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000a000000000000000000000000000000000000000000000000000000000000000e000000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000068d40fa100000000000000000000000000000000000000000000000000000000000000208e3ada0bc9a65c73374363655898f17ad104ea9822d37be8d954e72b2dcb0a36000000000000000000000000000000000000000000000000000000000000004e010000000000000000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000").unwrap();
    let schedule_payload_hash = solana_program::keccak::hashv(&[&schedule_payload]).to_bytes();

    // Cancel timelock proposal (command = 1)
    let cancel_timelock_payload: Vec<u8> = Vec::from_hex("0000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000a000000000000000000000000000000000000000000000000000000000000000e000000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000068d69bdc00000000000000000000000000000000000000000000000000000000000000208e3ada0bc9a65c73374363655898f17ad104ea9822d37be8d954e72b2dcb0a36000000000000000000000000000000000000000000000000000000000000004e010000000000000000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000").unwrap();
    let cancel_timelock_payload_hash =
        solana_program::keccak::hashv(&[&cancel_timelock_payload]).to_bytes();

    // Approve operator proposal (command = 2)
    let approve_operator_payload: Vec<u8> = Vec::from_hex("0000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000a000000000000000000000000000000000000000000000000000000000000000e000000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000068d40fa100000000000000000000000000000000000000000000000000000000000000208e3ada0bc9a65c73374363655898f17ad104ea9822d37be8d954e72b2dcb0a36000000000000000000000000000000000000000000000000000000000000004e010000000000000000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000").unwrap();
    let approve_operator_payload_hash =
        solana_program::keccak::hashv(&[&approve_operator_payload]).to_bytes();

    // Cancel operator approval (command = 3)
    let cancel_operator_payload: Vec<u8> = Vec::from_hex("0000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000300000000000000000000000000000000000000000000000000000000000000a000000000000000000000000000000000000000000000000000000000000000e000000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000068d40fa100000000000000000000000000000000000000000000000000000000000000208e3ada0bc9a65c73374363655898f17ad104ea9822d37be8d954e72b2dcb0a36000000000000000000000000000000000000000000000000000000000000004e010000000000000000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000").unwrap();
    let cancel_operator_payload_hash =
        solana_program::keccak::hashv(&[&cancel_operator_payload]).to_bytes();

    // Step 4: Create messages for all 4 commands
    let messages = vec![
        create_test_message(
            "ethereum",
            "schedule_msg",
            &GOVERNANCE_PROGRAM_ID.to_string(),
            schedule_payload_hash,
        ),
        create_test_message(
            "ethereum",
            "cancel_timelock_msg",
            &GOVERNANCE_PROGRAM_ID.to_string(),
            cancel_timelock_payload_hash,
        ),
        create_test_message(
            "ethereum",
            "approve_operator_msg",
            &GOVERNANCE_PROGRAM_ID.to_string(),
            approve_operator_payload_hash,
        ),
        create_test_message(
            "ethereum",
            "cancel_operator_msg",
            &GOVERNANCE_PROGRAM_ID.to_string(),
            cancel_operator_payload_hash,
        ),
    ];

    let incoming_messages = approve_messages_on_gateway(
        &setup,
        messages.clone(),
        init_result,
        &secret_key_1,
        &secret_key_2,
        verifier_leaves,
        verifier_merkle_tree,
    );

    // Step 10: Setup Governance
    setup.mollusk.add_program(
        &GOVERNANCE_PROGRAM_ID,
        "../../target/deploy/axelar_solana_governance_v2",
        &solana_sdk::bpf_loader_upgradeable::id(),
    );

    let payer = Pubkey::new_unique();
    let upgrade_authority = Pubkey::new_unique();
    let operator = Pubkey::new_unique();

    let (governance_config, governance_config_bump) = create_governance_config_pda();

    let program_data_pda = create_governance_program_data_pda();

    let (event_authority_pda_governance, event_authority_bump) =
        create_governance_event_authority_pda();

    let governance_setup = TestSetup {
        mollusk: setup.mollusk,
        payer,
        upgrade_authority,
        operator,
        governance_config,
        governance_config_bump,
        program_data_pda,
        event_authority_pda: event_authority_pda_governance,
        event_authority_bump,
    };

    let chain_hash = solana_program::keccak::hashv(&[b"ethereum"]).to_bytes();
    let address_hash =
        solana_program::keccak::hashv(&["0xSourceAddress".to_string().as_bytes()]).to_bytes();
    let minimum_proposal_eta_delay = 3600;

    let governance_config = GovernanceConfig::new(
        chain_hash,
        address_hash,
        minimum_proposal_eta_delay,
        operator.to_bytes(),
    );

    let init_governance_result =
        initialize_governance(&governance_setup, governance_config.clone());
    assert!(!init_governance_result.program_result.is_err());

    let governance_config_account = init_governance_result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == governance_setup.governance_config)
        .unwrap()
        .1
        .clone();

    let schedule_incoming_message = incoming_messages[0].clone().0;
    let schedule_incoming_message_pda = incoming_messages[0].1;
    let schedule_incoming_message_account_data = incoming_messages[0].clone().2;

    // Process SCHEDULE timelock proposal first
    let schedule_message = messages[0].clone();
    let schedule_signing_pda =
        create_signing_pda_from_message(&schedule_message, &schedule_incoming_message);
    let event_authority_pda_gateway = create_gateway_event_authority_pda();
    let proposal_hash = extract_proposal_hash_unchecked(&schedule_payload);
    let proposal_pda = create_proposal_pda(&proposal_hash);

    let gmp_context = GmpContext::new()
        .with_incoming_message(
            schedule_incoming_message_pda,
            schedule_incoming_message_account_data.clone(),
        )
        .with_governance_config(
            governance_setup.governance_config,
            governance_config_account.data.clone(),
        )
        .with_signing_pda(schedule_signing_pda)
        .with_event_authority_pda(event_authority_pda_gateway)
        .with_event_authority_pda_governance(event_authority_pda_governance)
        .with_proposal(proposal_pda, vec![], SYSTEM_PROGRAM_ID);

    // Send schedule timelock proposal
    let schedule_result = process_gmp_helper(
        &governance_setup,
        schedule_message,
        schedule_payload,
        gmp_context,
    );
    assert!(!schedule_result.program_result.is_err());

    let proposal_pda_account_after_schedule = schedule_result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == proposal_pda)
        .unwrap()
        .1
        .clone();

    let _ = ExecutableProposal::try_deserialize(
        &mut proposal_pda_account_after_schedule.data.as_slice(),
    )
    .unwrap();

    // Now process APPROVE operator proposal
    let approve_operator_message = messages[2].clone();
    let approve_operator_incoming_message = incoming_messages[2].clone().0;
    let approve_operator_incoming_message_pda = incoming_messages[2].1;
    let approve_operator_incoming_message_account_data = incoming_messages[2].clone().2;

    let approve_operator_signing_pda = create_signing_pda_from_message(
        &approve_operator_message,
        &approve_operator_incoming_message,
    );

    let operator_proposal_pda = create_operator_proposal_pda(&proposal_hash);

    let gmp_context = GmpContext::new()
        .with_incoming_message(
            approve_operator_incoming_message_pda,
            approve_operator_incoming_message_account_data,
        )
        .with_governance_config(
            governance_setup.governance_config,
            governance_config_account.data.clone(),
        )
        .with_signing_pda(approve_operator_signing_pda)
        .with_event_authority_pda(event_authority_pda_gateway)
        .with_event_authority_pda_governance(event_authority_pda_governance)
        .with_proposal(
            proposal_pda,
            proposal_pda_account_after_schedule.data,
            GOVERNANCE_PROGRAM_ID,
        )
        .with_operator_proposal(operator_proposal_pda, vec![], SYSTEM_PROGRAM_ID);

    // Send approve operator proposal
    let approve_operator_result = process_gmp_helper(
        &governance_setup,
        approve_operator_message,
        approve_operator_payload,
        gmp_context,
    );
    assert!(!approve_operator_result.program_result.is_err());

    let operator_proposal_pda_account = approve_operator_result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == operator_proposal_pda)
        .unwrap()
        .1
        .clone();

    assert!(
        !operator_proposal_pda_account.data.is_empty(),
        "Operator proposal PDA should be created"
    );

    let operator_proposal = axelar_solana_governance_v2::OperatorProposal::try_deserialize(
        &mut operator_proposal_pda_account.data.as_slice(),
    );
    assert!(
        operator_proposal.is_ok(),
        "Should be able to deserialize OperatorProposal"
    );

    let cancel_operator_incoming_message = incoming_messages[3].clone().0;
    let cancel_operator_incoming_message_pda = incoming_messages[3].1;
    let cancel_operator_incoming_message_account_data = incoming_messages[3].clone().2;

    // Now approve the CANCEL operator proposal message and process it
    let cancel_operator_message = messages[3].clone();
    let cancel_operator_signing_pda = create_signing_pda_from_message(
        &cancel_operator_message,
        &cancel_operator_incoming_message,
    );

    // Get the current proposal account data (after approve operator)
    let proposal_pda_account_after_approve = approve_operator_result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == proposal_pda)
        .unwrap()
        .1
        .clone();

    let gmp_context = GmpContext::new()
        .with_incoming_message(
            cancel_operator_incoming_message_pda,
            cancel_operator_incoming_message_account_data,
        )
        .with_governance_config(
            governance_setup.governance_config,
            governance_config_account.data,
        )
        .with_signing_pda(cancel_operator_signing_pda)
        .with_event_authority_pda(event_authority_pda_gateway)
        .with_event_authority_pda_governance(event_authority_pda_governance)
        .with_proposal(
            proposal_pda,
            proposal_pda_account_after_approve.data,
            GOVERNANCE_PROGRAM_ID,
        )
        .with_operator_proposal(
            operator_proposal_pda,
            operator_proposal_pda_account.data,
            GOVERNANCE_PROGRAM_ID,
        );

    //  Send CANCEL operator proposal
    let cancel_operator_result = process_gmp_helper(
        &governance_setup,
        cancel_operator_message,
        cancel_operator_payload,
        gmp_context,
    );
    assert!(!cancel_operator_result.program_result.is_err());

    // Verify that the operator proposal PDA was closed
    let operator_proposal_account_after_cancel = cancel_operator_result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == operator_proposal_pda);

    assert!(
        operator_proposal_account_after_cancel.is_none()
            || operator_proposal_account_after_cancel
                .unwrap()
                .1
                .data
                .is_empty(),
        "Operator proposal PDA should be closed after cancel"
    );

    // Verify the governance config account received the lamports from the closed account
    let governance_config_after_cancel = cancel_operator_result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == governance_setup.governance_config)
        .unwrap()
        .1
        .clone();

    assert!(
        governance_config_after_cancel.lamports >= governance_config_account.lamports,
        "Governance config should have received lamports from closed operator proposal account"
    );
}

#[test]
fn should_execute_scheduled_proposal() {
    // Step 1: Setup gateway with real signers
    let (mut setup, verifier_leaves, verifier_merkle_tree, secret_key_1, secret_key_2) =
        setup_test_with_real_signers();

    // Step 2: Initialize gateway
    let init_result = initialize_gateway(&setup);
    assert!(!init_result.program_result.is_err());

    // Step 3: Create the memo proposal data
    let memo = String::from("This is a sample memo");
    let native_value_u64 = 1;
    let eta = 1800000000;

    let value_receiver_pubkey = Pubkey::new_unique();

    let value_receiver = SolanaAccountMetadata {
        pubkey: value_receiver_pubkey.to_bytes(),
        is_signer: false,
        is_writable: false,
    };

    let call_data = get_memo_instruction_data(memo, value_receiver);
    let target_bytes: [u8; 32] = MEMO_PROGRAM_ID.to_bytes();
    let native_value = U256::from(native_value_u64);
    let eta = U256::from(eta);

    let gmp_payload = governance_gmp::GovernanceCommandPayload {
        command: governance_gmp::GovernanceCommand::ScheduleTimeLockProposal,
        target: target_bytes.to_vec().into(),
        call_data: call_data.try_to_vec().unwrap().into(),
        native_value,
        eta,
    };

    // Encode the GMP payload
    let schedule_payload = gmp_payload.abi_encode();
    let schedule_payload_hash = solana_program::keccak::hashv(&[&schedule_payload]).to_bytes();

    let other_payload: Vec<u8> = Vec::from_hex("DEADBEEF").unwrap();
    let other_payload_hash = solana_program::keccak::hashv(&[&other_payload]).to_bytes();

    let messages = vec![
        create_test_message(
            "ethereum",
            "msg_id_1",
            &GOVERNANCE_PROGRAM_ID.to_string(),
            schedule_payload_hash,
        ),
        create_test_message(
            "ethereum",
            "msg_id_2",
            &GOVERNANCE_PROGRAM_ID.to_string(),
            other_payload_hash,
        ),
    ];

    let incoming_messages = approve_messages_on_gateway(
        &setup,
        messages.clone(),
        init_result,
        &secret_key_1,
        &secret_key_2,
        verifier_leaves,
        verifier_merkle_tree,
    );

    // Now we have an approved message
    // Add remaining programs to mollusk
    setup.mollusk.add_program(
        &GOVERNANCE_PROGRAM_ID,
        "../../target/deploy/axelar_solana_governance_v2",
        &solana_sdk::bpf_loader_upgradeable::id(),
    );

    setup.mollusk.add_program(
        &MEMO_PROGRAM_ID,
        "../../target/deploy/axelar_solana_memo_v2",
        &solana_sdk::bpf_loader_upgradeable::id(),
    );

    let payer = Pubkey::new_unique();
    let upgrade_authority = Pubkey::new_unique();
    let operator = Pubkey::new_unique();

    let (governance_config_pda, governance_config_bump) =
        Pubkey::find_program_address(&[seed_prefixes::GOVERNANCE_CONFIG], &GOVERNANCE_PROGRAM_ID);

    let program_data_pda = create_governance_program_data_pda();

    let (event_authority_pda_governance, event_authority_bump) =
        create_governance_event_authority_pda();

    let chain_hash = solana_program::keccak::hashv(&[b"ethereum"]).to_bytes();
    let address_hash =
        solana_program::keccak::hashv(&["0xSourceAddress".to_string().as_bytes()]).to_bytes();
    let minimum_proposal_eta_delay = 3600;

    let mut governance_setup = TestSetup {
        mollusk: setup.mollusk,
        payer,
        upgrade_authority,
        operator,
        governance_config: governance_config_pda,
        governance_config_bump,
        program_data_pda,
        event_authority_pda: event_authority_pda_governance,
        event_authority_bump,
    };

    let governance_config = GovernanceConfig::new(
        chain_hash,
        address_hash,
        minimum_proposal_eta_delay,
        operator.to_bytes(),
    );

    let result = initialize_governance(&governance_setup, governance_config.clone());
    assert!(!result.program_result.is_err());

    let governance_config_account = result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == governance_setup.governance_config)
        .unwrap()
        .1
        .clone();

    let message = messages[0].clone();
    let schedule_incoming_message = incoming_messages[0].clone().0;
    let schedule_incoming_message_pda = incoming_messages[0].1;
    let schedule_incoming_message_account_data = incoming_messages[0].clone().2;

    let signing_pda = create_signing_pda_from_message(&message, &schedule_incoming_message);
    let event_authority_pda_gateway = create_gateway_event_authority_pda();
    let (event_authority_pda_governance, _) = create_governance_event_authority_pda();

    let proposal_hash = extract_proposal_hash_unchecked(&schedule_payload);
    let proposal_pda = create_proposal_pda(&proposal_hash);

    let gmp_context = GmpContext::new()
        .with_incoming_message(
            schedule_incoming_message_pda,
            schedule_incoming_message_account_data.clone(),
        )
        .with_governance_config(
            governance_setup.governance_config,
            governance_config_account.data.clone(),
        )
        .with_signing_pda(signing_pda)
        .with_event_authority_pda(event_authority_pda_gateway)
        .with_event_authority_pda_governance(event_authority_pda_governance)
        .with_proposal(proposal_pda, vec![], SYSTEM_PROGRAM_ID);

    // Send schedule timelock proposal
    let result = process_gmp_helper(
        &governance_setup,
        messages[0].clone(),
        schedule_payload,
        gmp_context,
    );
    assert!(!result.program_result.is_err());

    // Create execute proposal instruction discriminator
    let instruction_data = create_execute_proposal_instruction_data(
        MEMO_PROGRAM_ID.to_bytes(),
        call_data.clone(),
        native_value.to_le_bytes(),
    );

    // Get the updated governance config account
    let governance_config_account_updated = result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == governance_setup.governance_config)
        .unwrap()
        .1
        .clone();

    // Get the proposal PDA account after scheduling
    let proposal_pda_account_updated = result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == proposal_pda)
        .unwrap()
        .1
        .clone();

    // Set up accounts for execute proposal instruction
    let accounts = vec![
        (
            SYSTEM_PROGRAM_ID,
            Account {
                lamports: 1,
                data: vec![],
                owner: solana_sdk::native_loader::id(),
                executable: true,
                rent_epoch: 0,
            },
        ),
        (
            governance_setup.governance_config,
            governance_config_account_updated.clone(),
        ),
        (proposal_pda, proposal_pda_account_updated),
        (
            event_authority_pda_governance,
            Account {
                lamports: 0,
                data: vec![],
                owner: SYSTEM_PROGRAM_ID,
                executable: false,
                rent_epoch: 0,
            },
        ),
        (
            GOVERNANCE_PROGRAM_ID,
            Account {
                lamports: LAMPORTS_PER_SOL,
                data: vec![],
                owner: solana_sdk::bpf_loader_upgradeable::id(),
                executable: true,
                rent_epoch: 0,
            },
        ),
        // Remaining accounts
        (
            MEMO_PROGRAM_ID,
            Account {
                lamports: LAMPORTS_PER_SOL,
                data: vec![],
                owner: solana_sdk::bpf_loader_upgradeable::id(),
                executable: true,
                rent_epoch: 0,
            },
        ),
        (
            value_receiver_pubkey,
            Account {
                lamports: 0,
                data: vec![],
                owner: SYSTEM_PROGRAM_ID,
                executable: false,
                rent_epoch: 0,
            },
        ),
        (
            governance_setup.governance_config,
            governance_config_account_updated,
        ),
    ];

    let instruction = Instruction {
        program_id: GOVERNANCE_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
            AccountMeta::new(governance_setup.governance_config, false),
            AccountMeta::new(proposal_pda, false),
            // for emit cpi
            AccountMeta::new_readonly(event_authority_pda_governance, false),
            AccountMeta::new_readonly(GOVERNANCE_PROGRAM_ID, false),
            // Remaining accounts
            AccountMeta::new_readonly(MEMO_PROGRAM_ID, false),
            AccountMeta::new(value_receiver_pubkey, false),
            AccountMeta::new(governance_setup.governance_config, false),
        ],
        data: instruction_data,
    };

    // Convert eta from U256 to i64 for timestamp comparison
    let eta_timestamp: i64 = eta.try_into().unwrap_or(1800000000i64);

    // Make Mollusk think we're in the future by modifying its clock sysvar
    let current_timestamp = eta_timestamp + 3600; // 1 hour past ETA to be safe
    governance_setup.mollusk.sysvars.clock = Clock {
        slot: 1000,
        epoch_start_timestamp: eta_timestamp,
        epoch: 1,
        leader_schedule_epoch: 1,
        unix_timestamp: current_timestamp,
    };

    let governance_config_account_before = result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == governance_config_pda)
        .unwrap();

    let execute_result = governance_setup
        .mollusk
        .process_instruction(&instruction, &accounts);

    assert!(
        !execute_result.program_result.is_err(),
        "Execute proposal should succeed: {:?}",
        execute_result.program_result
    );

    // Verify the proposal PDA was closed
    let proposal_account_after_execution = execute_result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == proposal_pda)
        .unwrap();

    assert_eq!(proposal_account_after_execution.1.data.len(), 0);

    assert_eq!(
        proposal_account_after_execution.1.lamports, 0,
        "Proposal PDA should be closed after execution"
    );

    let value_receiver_account_after_execution = execute_result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == value_receiver_pubkey)
        .unwrap();

    let governance_config_account_after = execute_result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == governance_config_pda)
        .unwrap();

    // Governance config should get closed PDA lamports
    assert!(
        governance_config_account_after.1.lamports > governance_config_account_before.1.lamports
    );
    assert!(value_receiver_account_after_execution.1.lamports == native_value_u64);
}

#[test]
fn should_execute_operator_proposal() {
    // Step 1: Setup gateway with real signers
    let (mut setup, verifier_leaves, verifier_merkle_tree, secret_key_1, secret_key_2) =
        setup_test_with_real_signers();

    // Step 2: Initialize gateway
    let init_result = initialize_gateway(&setup);
    assert!(!init_result.program_result.is_err());

    // Step 3: Create governance message payloads for schedule and approve operator
    let memo = String::from("This is a operator proposal memo");
    let native_value_u64 = 2;
    let eta = 1800000000;

    let value_receiver_pubkey = Pubkey::new_unique();
    let value_receiver = SolanaAccountMetadata {
        pubkey: value_receiver_pubkey.to_bytes(),
        is_signer: false,
        is_writable: false,
    };
    let target_bytes: [u8; 32] = MEMO_PROGRAM_ID.to_bytes();
    let call_data = get_memo_instruction_data(memo, value_receiver);
    let native_value = U256::from(native_value_u64);
    let eta = U256::from(eta);

    // Schedule timelock proposal payload
    let schedule_gmp_payload = governance_gmp::GovernanceCommandPayload {
        command: governance_gmp::GovernanceCommand::ScheduleTimeLockProposal,
        target: target_bytes.to_vec().into(),
        call_data: call_data.try_to_vec().unwrap().into(),
        native_value,
        eta,
    };
    let schedule_payload = schedule_gmp_payload.abi_encode();
    let schedule_payload_hash = solana_program::keccak::hashv(&[&schedule_payload]).to_bytes();

    // Approve operator proposal payload (same target/call_data/native_value)
    let approve_operator_gmp_payload = governance_gmp::GovernanceCommandPayload {
        command: governance_gmp::GovernanceCommand::ApproveOperatorProposal,
        target: target_bytes.to_vec().into(),
        call_data: call_data.try_to_vec().unwrap().into(),
        native_value,
        eta,
    };
    let approve_operator_payload = approve_operator_gmp_payload.abi_encode();
    let approve_operator_payload_hash =
        solana_program::keccak::hashv(&[&approve_operator_payload]).to_bytes();

    let messages = vec![
        create_test_message(
            "ethereum",
            "schedule_msg",
            &GOVERNANCE_PROGRAM_ID.to_string(),
            schedule_payload_hash,
        ),
        create_test_message(
            "ethereum",
            "approve_operator_msg",
            &GOVERNANCE_PROGRAM_ID.to_string(),
            approve_operator_payload_hash,
        ),
    ];

    let incoming_messages = approve_messages_on_gateway(
        &setup,
        messages.clone(),
        init_result,
        &secret_key_1,
        &secret_key_2,
        verifier_leaves,
        verifier_merkle_tree,
    );

    // Step 7: Setup Governance
    setup.mollusk.add_program(
        &GOVERNANCE_PROGRAM_ID,
        "../../target/deploy/axelar_solana_governance_v2",
        &solana_sdk::bpf_loader_upgradeable::id(),
    );

    setup.mollusk.add_program(
        &MEMO_PROGRAM_ID,
        "../../target/deploy/axelar_solana_memo_v2",
        &solana_sdk::bpf_loader_upgradeable::id(),
    );

    let payer = Pubkey::new_unique();
    let upgrade_authority = Pubkey::new_unique();
    let operator = Pubkey::new_unique();

    let (governance_config, governance_config_bump) = create_governance_config_pda();
    let program_data_pda = create_governance_program_data_pda();
    let (event_authority_pda_governance, event_authority_bump) =
        create_governance_event_authority_pda();

    let governance_setup = TestSetup {
        mollusk: setup.mollusk,
        payer,
        upgrade_authority,
        operator,
        governance_config,
        governance_config_bump,
        program_data_pda,
        event_authority_pda: event_authority_pda_governance,
        event_authority_bump,
    };

    let chain_hash = solana_program::keccak::hashv(&[b"ethereum"]).to_bytes();
    let address_hash =
        solana_program::keccak::hashv(&["0xSourceAddress".to_string().as_bytes()]).to_bytes();
    let minimum_proposal_eta_delay = 3600;

    let governance_config_data = GovernanceConfig::new(
        chain_hash,
        address_hash,
        minimum_proposal_eta_delay,
        operator.to_bytes(),
    );

    let init_governance_result = initialize_governance(&governance_setup, governance_config_data);
    assert!(!init_governance_result.program_result.is_err());

    let governance_config_account = init_governance_result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == governance_setup.governance_config)
        .unwrap()
        .1
        .clone();

    // Step 8: Process SCHEDULE timelock proposal
    let schedule_message = messages[0].clone();
    let schedule_incoming_message_pda = incoming_messages[0].1;
    let schedule_incoming_message_account_data = incoming_messages[0].clone().2;

    let schedule_incoming_message =
        IncomingMessage::try_deserialize(&mut schedule_incoming_message_account_data.as_slice())
            .unwrap();

    let schedule_signing_pda =
        create_signing_pda_from_message(&schedule_message, &schedule_incoming_message);
    let event_authority_pda_gateway = create_gateway_event_authority_pda();
    let proposal_hash = extract_proposal_hash_unchecked(&schedule_payload);
    let proposal_pda = create_proposal_pda(&proposal_hash);

    let gmp_context = GmpContext::new()
        .with_incoming_message(
            schedule_incoming_message_pda,
            schedule_incoming_message_account_data.clone(),
        )
        .with_governance_config(
            governance_setup.governance_config,
            governance_config_account.data.clone(),
        )
        .with_signing_pda(schedule_signing_pda)
        .with_event_authority_pda(event_authority_pda_gateway)
        .with_event_authority_pda_governance(event_authority_pda_governance)
        .with_proposal(proposal_pda, vec![], SYSTEM_PROGRAM_ID);

    // Send schedule timelock proposal
    let schedule_result = process_gmp_helper(
        &governance_setup,
        schedule_message,
        schedule_payload,
        gmp_context,
    );
    assert!(!schedule_result.program_result.is_err());

    let proposal_pda_account_after_schedule = schedule_result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == proposal_pda)
        .unwrap()
        .1
        .clone();

    // Step 9: Process APPROVE operator proposal
    let approve_operator_message = messages[1].clone();
    let approve_operator_incoming_message_pda = incoming_messages[1].1;
    let approve_operator_incoming_message_account_data = incoming_messages[1].clone().2;

    let approve_operator_incoming_message = IncomingMessage::try_deserialize(
        &mut approve_operator_incoming_message_account_data.as_slice(),
    )
    .unwrap();

    let approve_operator_signing_pda = create_signing_pda_from_message(
        &approve_operator_message,
        &approve_operator_incoming_message,
    );

    let operator_proposal_pda = create_operator_proposal_pda(&proposal_hash);

    let gmp_context = GmpContext::new()
        .with_incoming_message(
            approve_operator_incoming_message_pda,
            approve_operator_incoming_message_account_data,
        )
        .with_governance_config(
            governance_setup.governance_config,
            governance_config_account.data.clone(),
        )
        .with_signing_pda(approve_operator_signing_pda)
        .with_event_authority_pda(event_authority_pda_gateway)
        .with_event_authority_pda_governance(event_authority_pda_governance)
        .with_proposal(
            proposal_pda,
            proposal_pda_account_after_schedule.data,
            GOVERNANCE_PROGRAM_ID,
        )
        .with_operator_proposal(operator_proposal_pda, vec![], SYSTEM_PROGRAM_ID);

    // Send approve operator proposal
    let approve_operator_result = process_gmp_helper(
        &governance_setup,
        approve_operator_message,
        approve_operator_payload,
        gmp_context,
    );
    assert!(!approve_operator_result.program_result.is_err());

    // Step 10: Execute the operator proposal
    let instruction_data = create_execute_operator_proposal_instruction_data(
        MEMO_PROGRAM_ID.to_bytes(),
        call_data.clone(),
        native_value.to_le_bytes(),
    );

    // Get updated accounts
    let governance_config_account_updated = approve_operator_result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == governance_setup.governance_config)
        .unwrap()
        .1
        .clone();

    let proposal_pda_account_updated = approve_operator_result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == proposal_pda)
        .unwrap()
        .1
        .clone();

    let operator_proposal_account_updated = approve_operator_result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == operator_proposal_pda)
        .unwrap()
        .1
        .clone();

    // Set up accounts for execute operator proposal instruction
    let accounts = vec![
        (
            SYSTEM_PROGRAM_ID,
            Account {
                lamports: 1,
                data: vec![],
                owner: solana_sdk::native_loader::id(),
                executable: true,
                rent_epoch: 0,
            },
        ),
        (
            governance_setup.governance_config,
            governance_config_account_updated.clone(),
        ),
        (proposal_pda, proposal_pda_account_updated),
        (
            operator,
            Account {
                lamports: LAMPORTS_PER_SOL,
                data: vec![],
                owner: SYSTEM_PROGRAM_ID,
                executable: false,
                rent_epoch: 0,
            },
        ),
        (operator_proposal_pda, operator_proposal_account_updated),
        (
            event_authority_pda_governance,
            Account {
                lamports: 0,
                data: vec![],
                owner: SYSTEM_PROGRAM_ID,
                executable: false,
                rent_epoch: 0,
            },
        ),
        (
            GOVERNANCE_PROGRAM_ID,
            Account {
                lamports: LAMPORTS_PER_SOL,
                data: vec![],
                owner: solana_sdk::bpf_loader_upgradeable::id(),
                executable: true,
                rent_epoch: 0,
            },
        ),
        // Remaining accounts
        (
            MEMO_PROGRAM_ID,
            Account {
                lamports: LAMPORTS_PER_SOL,
                data: vec![],
                owner: solana_sdk::bpf_loader_upgradeable::id(),
                executable: true,
                rent_epoch: 0,
            },
        ),
        (
            value_receiver_pubkey,
            Account {
                lamports: 0,
                data: vec![],
                owner: SYSTEM_PROGRAM_ID,
                executable: false,
                rent_epoch: 0,
            },
        ),
        (
            governance_setup.governance_config,
            governance_config_account_updated,
        ),
    ];

    let instruction = Instruction {
        program_id: GOVERNANCE_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
            AccountMeta::new(governance_setup.governance_config, false),
            AccountMeta::new(proposal_pda, false),
            AccountMeta::new_readonly(operator, true), // operator must sign
            AccountMeta::new(operator_proposal_pda, false),
            // for emit cpi
            AccountMeta::new_readonly(event_authority_pda_governance, false),
            AccountMeta::new_readonly(GOVERNANCE_PROGRAM_ID, false),
            // Remaining accounts
            AccountMeta::new_readonly(MEMO_PROGRAM_ID, false),
            AccountMeta::new(value_receiver_pubkey, false),
            AccountMeta::new(governance_setup.governance_config, false),
        ],
        data: instruction_data,
    };

    let execute_result = governance_setup
        .mollusk
        .process_instruction(&instruction, &accounts);

    assert!(
        !execute_result.program_result.is_err(),
        "Execute operator proposal should succeed: {:?}",
        execute_result.program_result
    );

    // Verify both PDAs were closed
    let proposal_account_after_execution = execute_result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == proposal_pda)
        .unwrap();

    assert_eq!(
        proposal_account_after_execution.1.data.len(),
        0,
        "Proposal PDA should be closed after execution"
    );
    assert_eq!(
        proposal_account_after_execution.1.lamports, 0,
        "Proposal PDA should have zero lamports after execution"
    );

    let operator_proposal_account_after_execution = execute_result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == operator_proposal_pda)
        .unwrap();

    assert_eq!(
        operator_proposal_account_after_execution.1.data.len(),
        0,
        "Operator proposal PDA should be closed after execution"
    );
    assert_eq!(
        operator_proposal_account_after_execution.1.lamports, 0,
        "Operator proposal PDA should have zero lamports after execution"
    );

    // Verify lamport transfer to value receiver
    let value_receiver_account_after_execution = execute_result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == value_receiver_pubkey)
        .unwrap();

    assert_eq!(
        value_receiver_account_after_execution.1.lamports, native_value_u64,
        "Value receiver should have received native value lamports"
    );

    // Verify governance config received the closed PDA lamports
    let governance_config_account_after = execute_result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == governance_config)
        .unwrap();

    assert!(
        governance_config_account_after.1.lamports > governance_config_account.lamports,
        "Governance config should have received lamports from closed PDAs"
    );
}

#[test]
fn should_transfer_operatorship() {
    let setup = mock_setup_test();
    let chain_hash = [1u8; 32];
    let address_hash = [2u8; 32];
    let minimum_proposal_eta_delay = 3600;

    let governance_config = GovernanceConfig::new(
        chain_hash,
        address_hash,
        minimum_proposal_eta_delay,
        setup.operator.to_bytes(),
    );

    let result = initialize_governance(&setup, governance_config.clone());
    assert!(!result.program_result.is_err());

    let new_operator = Pubkey::new_unique();
    let instruction_data = create_transfer_operatorship_instruction_data(new_operator);

    let governance_config_account = result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == setup.governance_config)
        .unwrap()
        .1
        .clone();

    // Set up accounts for transfer operatorship instruction
    let accounts = vec![
        (
            SYSTEM_PROGRAM_ID,
            Account {
                lamports: 1,
                data: vec![],
                owner: solana_sdk::native_loader::id(),
                executable: true,
                rent_epoch: 0,
            },
        ),
        (
            setup.operator,
            Account {
                lamports: LAMPORTS_PER_SOL,
                data: vec![],
                owner: SYSTEM_PROGRAM_ID,
                executable: false,
                rent_epoch: 0,
            },
        ),
        (setup.governance_config, governance_config_account),
        // For event CPI
        (
            setup.event_authority_pda,
            Account {
                lamports: 0,
                data: vec![],
                owner: SYSTEM_PROGRAM_ID,
                executable: false,
                rent_epoch: 0,
            },
        ),
        (
            GOVERNANCE_PROGRAM_ID,
            Account {
                lamports: LAMPORTS_PER_SOL,
                data: vec![],
                owner: solana_sdk::bpf_loader_upgradeable::id(),
                executable: true,
                rent_epoch: 0,
            },
        ),
    ];

    let instruction = Instruction {
        program_id: GOVERNANCE_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
            AccountMeta::new_readonly(setup.operator, true),
            AccountMeta::new(setup.governance_config, false),
            // For emit cpi
            AccountMeta::new_readonly(setup.event_authority_pda, false),
            AccountMeta::new_readonly(GOVERNANCE_PROGRAM_ID, false),
        ],
        data: instruction_data,
    };

    let transfer_result = setup.mollusk.process_instruction(&instruction, &accounts);

    assert!(
        !transfer_result.program_result.is_err(),
        "Transfer operatorship should succeed: {:?}",
        transfer_result.program_result
    );

    // Verify the operator was changed
    let updated_governance_config_account = transfer_result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == setup.governance_config)
        .unwrap()
        .1
        .clone();

    let updated_config =
        GovernanceConfig::try_deserialize(&mut updated_governance_config_account.data.as_slice())
            .unwrap();

    assert_eq!(
        updated_config.operator,
        new_operator.to_bytes(),
        "Operator should have been updated to the new operator"
    );

    // Original config should remain the same except for operator
    assert_eq!(updated_config.chain_hash, governance_config.chain_hash);
    assert_eq!(updated_config.address_hash, governance_config.address_hash);
    assert_eq!(
        updated_config.minimum_proposal_eta_delay,
        governance_config.minimum_proposal_eta_delay
    );
}

#[test]
fn should_execute_withdraw_tokens_through_proposal() {
    // Step 1: Setup gateway with real signers
    let (mut setup, verifier_leaves, verifier_merkle_tree, secret_key_1, secret_key_2) =
        setup_test_with_real_signers();

    // Step 2: Initialize gateway
    let init_result = initialize_gateway(&setup);
    assert!(!init_result.program_result.is_err());

    // Step 3: Create the withdraw tokens proposal data
    let withdraw_amount = 5_000_000u64; // 0.005 SOL
    let native_value_u64 = 0;
    let eta = 1800000000;

    let receiver_pubkey = Pubkey::new_unique();
    let payer = Pubkey::new_unique();
    let upgrade_authority = Pubkey::new_unique();
    let operator = Pubkey::new_unique();

    let (governance_config_pda, governance_config_bump) =
        Pubkey::find_program_address(&[seed_prefixes::GOVERNANCE_CONFIG], &GOVERNANCE_PROGRAM_ID);

    let governance_config_bytes: [u8; 32] = governance_config_pda.to_bytes();
    let native_value = U256::from(native_value_u64);
    let eta = U256::from(eta);

    let call_data = get_withdraw_tokens_instruction_data(
        withdraw_amount,
        receiver_pubkey,
        governance_config_bytes,
    );

    // We want our proposal to execute on Governance itself
    let target_program_bytes = GOVERNANCE_PROGRAM_ID.to_bytes();

    let gmp_payload = governance_gmp::GovernanceCommandPayload {
        command: governance_gmp::GovernanceCommand::ScheduleTimeLockProposal,
        target: target_program_bytes.to_vec().into(),
        call_data: call_data.try_to_vec().unwrap().into(),
        native_value,
        eta,
    };

    let schedule_payload = gmp_payload.abi_encode();
    let schedule_payload_hash = solana_program::keccak::hashv(&[&schedule_payload]).to_bytes();

    let messages = vec![create_test_message(
        "ethereum",
        "withdraw_msg",
        &GOVERNANCE_PROGRAM_ID.to_string(),
        schedule_payload_hash,
    )];

    let incoming_messages = approve_messages_on_gateway(
        &setup,
        messages.clone(),
        init_result,
        &secret_key_1,
        &secret_key_2,
        verifier_leaves,
        verifier_merkle_tree,
    );

    let incoming_message = incoming_messages[0].clone();

    // Step 9: Setup Governance
    setup.mollusk.add_program(
        &GOVERNANCE_PROGRAM_ID,
        "../../target/deploy/axelar_solana_governance_v2",
        &solana_sdk::bpf_loader_upgradeable::id(),
    );

    let program_data_pda = create_governance_program_data_pda();
    let (event_authority_pda_governance, event_authority_bump) =
        create_governance_event_authority_pda();

    let chain_hash = solana_program::keccak::hashv(&[b"ethereum"]).to_bytes();
    let address_hash =
        solana_program::keccak::hashv(&["0xSourceAddress".to_string().as_bytes()]).to_bytes();
    let minimum_proposal_eta_delay = 3600;

    let mut governance_setup = TestSetup {
        mollusk: setup.mollusk,
        payer,
        upgrade_authority,
        operator,
        governance_config: governance_config_pda,
        governance_config_bump,
        program_data_pda,
        event_authority_pda: event_authority_pda_governance,
        event_authority_bump,
    };

    let governance_config_data = GovernanceConfig::new(
        chain_hash,
        address_hash,
        minimum_proposal_eta_delay,
        operator.to_bytes(),
    );

    let result = initialize_governance(&governance_setup, governance_config_data);
    assert!(!result.program_result.is_err());

    // Give governance config some lamports to withdraw
    let mut governance_config_account = result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == governance_setup.governance_config)
        .unwrap()
        .1
        .clone();

    let treasury_amount = 10_000_000u64; // Give it 0.01 SOL
    governance_config_account.lamports += treasury_amount;

    // Step 10: Schedule the withdraw proposal
    let message = messages[0].clone();

    let signing_pda = create_signing_pda_from_message(&message, &incoming_message.0.clone());
    let event_authority_pda_gateway = create_gateway_event_authority_pda();
    let proposal_hash = extract_proposal_hash_unchecked(&schedule_payload);
    let proposal_pda = create_proposal_pda(&proposal_hash);

    let gmp_context = GmpContext::new()
        .with_incoming_message(incoming_message.1, incoming_message.2.clone())
        .with_governance_config(
            governance_setup.governance_config,
            governance_config_account.data.clone(),
        )
        .with_signing_pda(signing_pda)
        .with_event_authority_pda(event_authority_pda_gateway)
        .with_event_authority_pda_governance(event_authority_pda_governance)
        .with_proposal(proposal_pda, vec![], SYSTEM_PROGRAM_ID);

    // Send schedule timelock proposal
    let schedule_result = process_gmp_helper(
        &governance_setup,
        messages[0].clone(),
        schedule_payload,
        gmp_context,
    );
    assert!(!schedule_result.program_result.is_err());

    // Step 11: Execute the proposal after ETA
    let instruction_data = create_execute_proposal_instruction_data(
        GOVERNANCE_PROGRAM_ID.to_bytes(),
        call_data.clone(),
        native_value.to_le_bytes(),
    );

    // Get the updated accounts
    let governance_config_account_updated = schedule_result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == governance_setup.governance_config)
        .unwrap()
        .1
        .clone();

    let proposal_pda_account_updated = schedule_result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == proposal_pda)
        .unwrap()
        .1
        .clone();

    let initial_governance_lamports = governance_config_account_updated.lamports;

    // Set up accounts for execute proposal instruction
    let accounts = vec![
        (
            SYSTEM_PROGRAM_ID,
            Account {
                lamports: 1,
                data: vec![],
                owner: solana_sdk::native_loader::id(),
                executable: true,
                rent_epoch: 0,
            },
        ),
        (
            governance_setup.governance_config,
            governance_config_account_updated.clone(),
        ),
        (proposal_pda, proposal_pda_account_updated),
        (
            event_authority_pda_governance,
            Account {
                lamports: 0,
                data: vec![],
                owner: SYSTEM_PROGRAM_ID,
                executable: false,
                rent_epoch: 0,
            },
        ),
        (
            GOVERNANCE_PROGRAM_ID,
            Account {
                lamports: LAMPORTS_PER_SOL,
                data: vec![],
                owner: solana_sdk::bpf_loader_upgradeable::id(),
                executable: true,
                rent_epoch: 0,
            },
        ),
        // Remaining accounts for withdraw_tokens instruction:
        (
            GOVERNANCE_PROGRAM_ID,
            Account {
                lamports: LAMPORTS_PER_SOL,
                data: vec![],
                owner: solana_sdk::bpf_loader_upgradeable::id(),
                executable: true,
                rent_epoch: 0,
            },
        ),
        (
            SYSTEM_PROGRAM_ID,
            Account {
                lamports: 1,
                data: vec![],
                owner: solana_sdk::native_loader::id(),
                executable: true,
                rent_epoch: 0,
            },
        ),
        (
            governance_setup.governance_config,
            governance_config_account_updated,
        ),
        (
            receiver_pubkey,
            Account {
                lamports: 0,
                data: vec![],
                owner: SYSTEM_PROGRAM_ID,
                executable: false,
                rent_epoch: 0,
            },
        ),
    ];

    let instruction = Instruction {
        program_id: GOVERNANCE_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
            AccountMeta::new(governance_setup.governance_config, false),
            AccountMeta::new(proposal_pda, false),
            // for event cpi
            AccountMeta::new_readonly(event_authority_pda_governance, false),
            AccountMeta::new_readonly(GOVERNANCE_PROGRAM_ID, false),
            // Remaining accounts (will be used in execution)
            AccountMeta::new_readonly(GOVERNANCE_PROGRAM_ID, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
            AccountMeta::new(governance_setup.governance_config, false),
            AccountMeta::new(receiver_pubkey, false),
        ],
        data: instruction_data,
    };

    // Set clock to after ETA
    let eta_timestamp: i64 = eta.try_into().unwrap_or(1800000000i64);
    let current_timestamp = eta_timestamp + 3600; // 1 hour past ETA
    governance_setup.mollusk.sysvars.clock = Clock {
        slot: 1000,
        epoch_start_timestamp: eta_timestamp,
        epoch: 1,
        leader_schedule_epoch: 1,
        unix_timestamp: current_timestamp,
    };

    let execute_result = governance_setup
        .mollusk
        .process_instruction(&instruction, &accounts);

    assert!(
        !execute_result.program_result.is_err(),
        "Execute withdraw proposal should succeed: {:?}",
        execute_result.program_result
    );

    // Verify the proposal PDA was closed
    let proposal_account_after_execution = execute_result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == proposal_pda)
        .unwrap();

    assert_eq!(proposal_account_after_execution.1.data.len(), 0);
    assert_eq!(proposal_account_after_execution.1.lamports, 0);

    // Verify the receiver got the withdrawn amount
    let receiver_account_after = execute_result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == receiver_pubkey)
        .unwrap();

    assert_eq!(
        receiver_account_after.1.lamports, withdraw_amount,
        "Receiver should have received the withdrawn amount"
    );

    // Verify governance config lost the withdrawn amount
    let governance_config_after = execute_result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == governance_setup.governance_config)
        .unwrap();

    assert!(governance_config_after.1.lamports < initial_governance_lamports);
}
