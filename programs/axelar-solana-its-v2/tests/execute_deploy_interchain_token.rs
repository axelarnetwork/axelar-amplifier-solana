use anchor_lang::{AccountDeserialize, InstructionData, ToAccountMetas};
use anchor_spl::{
    associated_token::spl_associated_token_account,
    token_2022::spl_token_2022::{self},
};
use axelar_solana_gateway_v2::{GatewayConfig, ID as GATEWAY_PROGRAM_ID};
use axelar_solana_gateway_v2_test_fixtures::{
    approve_messages_on_gateway, create_test_message, initialize_gateway,
    setup_test_with_real_signers,
};
use axelar_solana_its_v2::{state::TokenManager, utils::interchain_token_id};
use axelar_solana_its_v2_test_fixtures::{
    create_rent_sysvar_data, create_sysvar_instructions_data,
    init_its_service_with_ethereum_trusted, initialize_mollusk,
};
use interchain_token_transfer_gmp::{DeployInterchainToken, GMPPayload, ReceiveFromHub};
use mollusk_svm::program::keyed_account_for_system_program;
use mollusk_test_utils::get_event_authority_and_program_accounts;
use solana_program::program_pack::{IsInitialized, Pack};
use solana_sdk::{
    account::Account, instruction::Instruction, keccak, native_token::LAMPORTS_PER_SOL,
    pubkey::Pubkey,
};
use spl_token_2022::{extension::StateWithExtensions, state::Account as Token2022Account};

#[test]
fn test_execute_deploy_interchain_token_success() {
    // Step 1: Setup gateway with real signers
    let (mut setup, verifier_leaves, verifier_merkle_tree, secret_key_1, secret_key_2) =
        setup_test_with_real_signers();

    // Step 2: Initialize gateway
    let init_result = initialize_gateway(&setup);
    let (gateway_root_pda, _) = GatewayConfig::find_pda();
    let gateway_root_pda_account = init_result.get_account(&gateway_root_pda).unwrap();

    assert!(init_result.program_result.is_ok());

    // Step 3: Add ITS program to mollusk - USE the properly configured mollusk
    let program_id = axelar_solana_its_v2::id();

    // Use the properly configured mollusk that has Token2022 and other programs
    let mut mollusk = initialize_mollusk();

    // We still need to add the gateway program since initialize_mollusk doesn't include it for execution tests
    mollusk.add_program(
        &GATEWAY_PROGRAM_ID,
        "../../target/deploy/axelar_solana_gateway_v2",
        &solana_sdk::bpf_loader_upgradeable::id(),
    );

    // Update setup to use our properly configured mollusk
    setup.mollusk = mollusk;

    // Step 4: Initialize ITS service
    let payer = Pubkey::new_unique();
    let payer_account = Account::new(10 * LAMPORTS_PER_SOL, 0, &solana_sdk::system_program::ID);

    let operator = Pubkey::new_unique();
    let operator_account = Account::new(1_000_000_000, 0, &solana_sdk::system_program::ID);

    let chain_name = "solana".to_string();
    let its_hub_address = "0x123456789abcdef".to_string();

    // Use our configured mollusk for ITS service initialization too
    let (its_root_pda, its_root_account) = init_its_service_with_ethereum_trusted(
        &setup.mollusk, // Now this mollusk has Token2022 properly configured
        payer,
        &payer_account,
        payer,
        operator,
        &operator_account,
        chain_name.clone(),
        its_hub_address.clone(),
    );

    // Step 5: Create token deployment parameters
    let salt = [1u8; 32];
    let token_id = interchain_token_id(&payer, &salt);
    let name = "Test Token".to_string();
    let symbol = "TEST".to_string();
    let decimals = 9u8;

    // Step 6: Create the GMP payload
    let deploy_payload = DeployInterchainToken {
        selector: alloy_primitives::U256::from(1), // MESSAGE_TYPE_ID for DeployInterchainToken
        token_id: alloy_primitives::FixedBytes::from(token_id),
        name: name.clone(),
        symbol: symbol.clone(),
        decimals,
        minter: alloy_primitives::Bytes::new(), // Empty bytes for no minter
    };

    // Wrap in ReceiveFromHub payload
    let receive_from_hub_payload = ReceiveFromHub {
        selector: alloy_primitives::U256::from(4), // MESSAGE_TYPE_ID for ReceiveFromHub
        source_chain: "ethereum".to_string(),
        payload: GMPPayload::DeployInterchainToken(deploy_payload)
            .encode()
            .into(),
    };

    let gmp_payload = GMPPayload::ReceiveFromHub(receive_from_hub_payload);
    let encoded_payload = gmp_payload.encode();
    let payload_hash = keccak::hashv(&[&encoded_payload]).to_bytes();

    // Step 7: Create test message
    let message = create_test_message(
        "ethereum",
        "deploy_token_123",
        &program_id.to_string(),
        payload_hash,
    );

    // Override the source_address to match its_hub_address
    let mut message = message;
    message.source_address = its_hub_address.clone();

    // Step 8: Approve message on gateway
    let incoming_messages = approve_messages_on_gateway(
        &setup,
        vec![message.clone()],
        init_result.clone(),
        &secret_key_1,
        &secret_key_2,
        verifier_leaves,
        verifier_merkle_tree,
    );

    let (_, incoming_message_pda, incoming_message_account_data) = &incoming_messages[0];

    // Step 9: Find required PDAs - FIXED with correct seeds
    let (token_manager_pda, _) = Pubkey::find_program_address(
        &[
            axelar_solana_its_v2::seed_prefixes::TOKEN_MANAGER_SEED,
            its_root_pda.as_ref(),
            &token_id,
        ],
        &program_id,
    );

    let (token_mint_pda, _) = Pubkey::find_program_address(
        &[
            axelar_solana_its_v2::seed_prefixes::INTERCHAIN_TOKEN_SEED,
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

    let (signing_pda, _) = Pubkey::find_program_address(
        &[
            axelar_solana_gateway_v2::seed_prefixes::VALIDATE_MESSAGE_SIGNING_SEED,
            message.command_id().as_ref(),
        ],
        &program_id, // The caller program (ITS)
    );

    let (gateway_event_authority, _) =
        Pubkey::find_program_address(&[b"__event_authority"], &GATEWAY_PROGRAM_ID);

    // Fix: get_event_authority_and_program_accounts returns 3 elements
    let (its_event_authority, _event_authority_account, its_program_account) =
        get_event_authority_and_program_accounts(&program_id);

    // Step 10: Create execute instruction
    let instruction_data = axelar_solana_its_v2::instruction::Execute {
        message: message.clone(),
        payload: encoded_payload,
    };

    let executable_accounts = axelar_solana_its_v2::accounts::AxelarExecuteAccounts {
        incoming_message_pda: *incoming_message_pda,
        signing_pda,
        gateway_root_pda,
        event_authority: gateway_event_authority,
        axelar_gateway_program: GATEWAY_PROGRAM_ID,
    };

    let accounts = axelar_solana_its_v2::accounts::Execute {
        // GMP accounts
        executable: executable_accounts,

        // ITS accounts
        payer,
        its_root_pda,
        token_manager_pda,
        token_mint: token_mint_pda,
        token_manager_ata,
        token_program: spl_token_2022::id(),
        associated_token_program: spl_associated_token_account::id(),
        system_program: solana_sdk::system_program::ID,

        // Remaining accounts
        deployer_ata: Some(deployer_ata),
        minter: None,
        minter_roles_pda: None,
        mpl_token_metadata_account: Some(metadata_account),
        mpl_token_metadata_program: Some(mpl_token_metadata::ID),
        sysvar_instructions: Some(solana_sdk::sysvar::instructions::ID),
        destination: None,
        deployer: Some(payer),
        authority: None,
        destination_ata: None,

        // Event CPI accounts
        event_authority: its_event_authority,
        program: program_id,
    };

    let execute_instruction = Instruction {
        program_id,
        accounts: accounts.to_account_metas(None),
        data: instruction_data.data(),
    };

    let incoming_message_account = Account {
        lamports: setup
            .mollusk
            .sysvars
            .rent
            .minimum_balance(incoming_message_account_data.len()),
        data: incoming_message_account_data.clone(),
        owner: GATEWAY_PROGRAM_ID,
        executable: false,
        rent_epoch: 0,
    };

    let execute_accounts = vec![
        // AxelarExecuteAccounts
        (*incoming_message_pda, incoming_message_account),
        (
            signing_pda,
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
        (its_root_pda, its_root_account),
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
        // Remaining accounts
        (
            deployer_ata,
            Account::new(0, 0, &solana_sdk::system_program::ID),
        ),
        (program_id, its_program_account.clone()), // minter: None
        (program_id, its_program_account.clone()), // minter_roles_pda: None
        (
            metadata_account,
            Account::new(0, 0, &solana_sdk::system_program::ID),
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
            solana_sdk::sysvar::instructions::ID,
            Account {
                lamports: 1_000_000_000,
                data: create_sysvar_instructions_data(),
                owner: solana_program::sysvar::id(),
                executable: false,
                rent_epoch: 0,
            },
        ),
        (program_id, its_program_account.clone()), // destination -> None
        (payer, payer_account),                    // deployer is also payer
        (program_id, its_program_account.clone()), // authority -> None
        (program_id, its_program_account.clone()), // destination_ata -> None
        // Event CPI accounts
        (its_event_authority, _event_authority_account),
        (program_id, its_program_account.clone()),
    ];

    // Step 12: Execute the instruction
    let result = setup
        .mollusk
        .process_instruction(&execute_instruction, &execute_accounts);

    // Step 13: Verify success
    assert!(
        result.program_result.is_ok(),
        "Execute instruction should succeed: {:?}",
        result.program_result
    );

    let token_manager_account = result.get_account(&token_manager_pda).unwrap();
    let token_manager =
        TokenManager::try_deserialize(&mut token_manager_account.data.as_ref()).unwrap();

    assert_eq!(token_manager.token_id, token_id);

    let token_mint_account = result.get_account(&token_mint_pda).unwrap();
    let token_mint = spl_token_2022::state::Mint::unpack(&token_mint_account.data).unwrap();

    assert_eq!(token_mint.mint_authority, Some(token_manager_pda).into(),);
    assert_eq!(token_mint.freeze_authority, Some(token_manager_pda).into(),);
    assert_eq!(token_mint.decimals, decimals,);
    assert_eq!(token_mint.supply, 0, "Initial supply should be 0");
    assert!(token_mint.is_initialized);

    let metadata_acc = result.get_account(&metadata_account).unwrap();
    let metadata = mpl_token_metadata::accounts::Metadata::from_bytes(&metadata_acc.data).unwrap();

    assert_eq!(metadata.mint, token_mint_pda,);
    assert_eq!(metadata.update_authority, token_manager_pda,);

    let metadata_name = metadata.name.trim_end_matches('\0');
    let metadata_symbol = metadata.symbol.trim_end_matches('\0');

    assert_eq!(metadata_name, name,);
    assert_eq!(metadata_symbol, symbol);

    let deployer_ata_account = result.get_account(&deployer_ata).unwrap();
    let deployer_ata_data =
        StateWithExtensions::<Token2022Account>::unpack(&deployer_ata_account.data).unwrap();

    assert_eq!(deployer_ata_data.base.mint, token_mint_pda,);
    assert_eq!(deployer_ata_data.base.owner, payer,);
    assert_eq!(deployer_ata_data.base.amount, 0,);
    assert!(deployer_ata_data.base.is_initialized());
}
