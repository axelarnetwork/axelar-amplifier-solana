use anchor_lang::{AccountDeserialize, InstructionData, ToAccountMetas};
use anchor_spl::{associated_token::spl_associated_token_account, token_2022::spl_token_2022};
use axelar_solana_gateway_v2::{GatewayConfig, ID as GATEWAY_PROGRAM_ID};
use axelar_solana_gateway_v2_test_fixtures::{
    approve_messages_on_gateway, create_test_message, initialize_gateway,
    setup_test_with_real_signers,
};
use axelar_solana_its_v2::{state::TokenManager, utils::interchain_token_id};
use axelar_solana_its_v2_test_fixtures::{
    create_rent_sysvar_data, init_its_service_with_ethereum_trusted, initialize_mollusk,
};
use interchain_token_transfer_gmp::{GMPPayload, LinkToken, ReceiveFromHub};
use mollusk_svm::program::keyed_account_for_system_program;
use mollusk_test_utils::get_event_authority_and_program_accounts;
use solana_program::program_pack::Pack;
use solana_sdk::{
    account::Account, instruction::Instruction, keccak, native_token::LAMPORTS_PER_SOL,
    pubkey::Pubkey,
};

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
fn test_execute_link_token() {
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

    // Step 5: Create existing token mint (key difference from deploy test)
    let mint_authority = payer; // Use payer as mint authority for simplicity
    let (existing_token_mint, existing_token_mint_account) = create_test_mint(mint_authority);

    // Step 6: Create token linking parameters
    let salt = [1u8; 32];
    let token_id = interchain_token_id(&payer, &salt);
    let token_manager_type = 1u8; // LockUnlock type
    let source_token_address = existing_token_mint.to_bytes().to_vec();
    let destination_token_address = vec![0x12u8; 32]; // Must be 32 bytes, not 20!
    let link_params = vec![]; // No additional params (no operator)

    // Step 7: Create the GMP payload
    let link_payload = LinkToken {
        selector: alloy_primitives::U256::from(5), // MESSAGE_TYPE_ID for LinkToken
        token_id: alloy_primitives::FixedBytes::from(token_id),
        token_manager_type: alloy_primitives::U256::from(token_manager_type),
        source_token_address: alloy_primitives::Bytes::from(source_token_address),
        destination_token_address: alloy_primitives::Bytes::from(destination_token_address),
        link_params: alloy_primitives::Bytes::from(link_params.clone()),
    };

    // Wrap in ReceiveFromHub payload
    let receive_from_hub_payload = ReceiveFromHub {
        selector: alloy_primitives::U256::from(4), // MESSAGE_TYPE_ID for ReceiveFromHub
        source_chain: "ethereum".to_string(),
        payload: GMPPayload::LinkToken(link_payload).encode().into(),
    };

    let gmp_payload = GMPPayload::ReceiveFromHub(receive_from_hub_payload);
    let encoded_payload = gmp_payload.encode();
    let payload_hash = keccak::hashv(&[&encoded_payload]).to_bytes();

    // Step 8: Create test message
    let message = create_test_message(
        "ethereum",
        "link_token_123",
        &program_id.to_string(),
        payload_hash,
    );

    // Override the source_address to match its_hub_address
    let mut message = message;
    message.source_address = its_hub_address.clone();

    // Step 9: Approve message on gateway
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

    // Step 10: Find required PDAs
    let (token_manager_pda, _) = Pubkey::find_program_address(
        &[
            axelar_solana_its_v2::seed_prefixes::TOKEN_MANAGER_SEED,
            its_root_pda.as_ref(),
            &token_id,
        ],
        &program_id,
    );

    // For link token, we use the existing mint, not a new PDA
    let token_mint_pda = existing_token_mint;

    let (token_manager_ata, _) = Pubkey::find_program_address(
        &[
            token_manager_pda.as_ref(),
            spl_token_2022::id().as_ref(),
            token_mint_pda.as_ref(),
        ],
        &spl_associated_token_account::id(),
    );

    let (signing_pda, _) = Pubkey::find_program_address(
        &[
            axelar_solana_gateway_v2::seed_prefixes::VALIDATE_MESSAGE_SIGNING_SEED,
            message.command_id().as_ref(),
        ],
        &program_id,
    );

    let (gateway_event_authority, _) =
        Pubkey::find_program_address(&[b"__event_authority"], &GATEWAY_PROGRAM_ID);

    let (its_event_authority, _event_authority_account, its_program_account) =
        get_event_authority_and_program_accounts(&program_id);

    let instruction_data = axelar_solana_its_v2::instruction::Execute {
        message: message.clone(),
        payload: encoded_payload,
    };

    let executable_accounts = axelar_solana_its_v2::accounts::AxelarExecuteAccounts {
        incoming_message_pda: *incoming_message_pda,
        signing_pda,
        gateway_root_pda,
        axelar_gateway_program: GATEWAY_PROGRAM_ID,
        event_authority: gateway_event_authority,
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
        deployer_ata: None,
        minter: None,
        minter_roles_pda: None,
        mpl_token_metadata_account: None,
        mpl_token_metadata_program: None,
        sysvar_instructions: None,
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
        (token_mint_pda, existing_token_mint_account),
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
        (program_id, its_program_account.clone()),
        (program_id, its_program_account.clone()),
        (program_id, its_program_account.clone()),
        (program_id, its_program_account.clone()),
        (program_id, its_program_account.clone()),
        (program_id, its_program_account.clone()),
        (program_id, its_program_account.clone()),
        (payer, payer_account), // deployer same as payer for simplicity
        (program_id, its_program_account.clone()),
        (program_id, its_program_account.clone()),
        // Event CPI accounts
        (its_event_authority, _event_authority_account),
        (program_id, its_program_account.clone()),
    ];

    let result = setup
        .mollusk
        .process_instruction(&execute_instruction, &execute_accounts);

    assert!(result.program_result.is_ok());

    let token_manager_account = result.get_account(&token_manager_pda).unwrap();
    let token_manager =
        TokenManager::try_deserialize(&mut token_manager_account.data.as_ref()).unwrap();

    assert_eq!(token_manager.token_id, token_id,);
}
