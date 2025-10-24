use anchor_lang::{AccountDeserialize, AnchorDeserialize, InstructionData, ToAccountMetas};
use anchor_spl::{
    associated_token::get_associated_token_address_with_program_id,
    token::spl_token,
    token_2022::spl_token_2022::{self, extension::StateWithExtensions},
};
use axelar_solana_its_v2::{
    instructions::DeployInterchainTokenData,
    seed_prefixes::{INTERCHAIN_TOKEN_SEED, TOKEN_MANAGER_SEED},
    state::TokenManager,
    utils::{interchain_token_deployer_salt, interchain_token_id, interchain_token_id_internal},
};
use mollusk_svm_programs_token;
use mollusk_test_utils::{get_event_authority_and_program_accounts, setup_mollusk};
use solana_program::program_pack::Pack;
use solana_sdk::{
    account::Account, instruction::Instruction, native_token::LAMPORTS_PER_SOL, pubkey::Pubkey,
    system_program,
};
use spl_token_2022::state::Account as Token2022Account;

mod initialize;
use initialize::init_its_service;

fn create_rent_sysvar_data() -> Vec<u8> {
    use solana_sdk::rent::Rent;

    let rent = Rent::default();
    bincode::serialize(&rent).unwrap()
}

fn create_sysvar_instructions_data() -> Vec<u8> {
    use solana_sdk::sysvar::instructions::{construct_instructions_data, BorrowedInstruction};

    let instructions: &[BorrowedInstruction] = &[];
    construct_instructions_data(instructions)
}

#[test]
fn test_deploy_interchain_token() {
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
    ) = init_its_service(
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
    let minter = None;

    let deploy_params = DeployInterchainTokenData {
        salt,
        name: name.clone(),
        symbol: symbol.clone(),
        decimals,
        initial_supply,
        minter,
    };

    let token_id = interchain_token_id(&deployer, &salt);

    let (token_manager_pda, _) = Pubkey::find_program_address(
        &[TOKEN_MANAGER_SEED, its_root_pda.as_ref(), &token_id],
        &program_id,
    );

    let (token_mint_pda, _) = Pubkey::find_program_address(
        &[INTERCHAIN_TOKEN_SEED, its_root_pda.as_ref(), &token_id],
        &program_id,
    );

    let token_manager_ata = get_associated_token_address_with_program_id(
        &token_manager_pda,
        &token_mint_pda,
        &spl_token_2022::ID,
    );

    let deployer_ata = get_associated_token_address_with_program_id(
        &deployer,
        &token_mint_pda,
        &spl_token_2022::ID,
    );

    let (metadata_account, _) = Pubkey::find_program_address(
        &[
            b"metadata",
            mpl_token_metadata::programs::MPL_TOKEN_METADATA_ID.as_ref(),
            token_mint_pda.as_ref(),
        ],
        &mpl_token_metadata::programs::MPL_TOKEN_METADATA_ID,
    );

    let (event_authority, event_authority_account, program_account) =
        get_event_authority_and_program_accounts(&program_id);

    let ix_data = axelar_solana_its_v2::instruction::DeployInterchainToken {
        params: deploy_params,
    }
    .data();

    let ix = Instruction {
        program_id,
        accounts: axelar_solana_its_v2::accounts::DeployInterchainToken {
            payer,
            deployer,
            system_program: system_program::ID,
            its_root_pda,
            token_manager_pda,
            token_mint: token_mint_pda,
            token_manager_ata,
            token_program: spl_token_2022::ID,
            associated_token_program: anchor_spl::associated_token::ID,
            rent: solana_sdk::sysvar::rent::ID,
            sysvar_instructions: solana_sdk::sysvar::instructions::ID,
            mpl_token_metadata_program: mpl_token_metadata::programs::MPL_TOKEN_METADATA_ID,
            mpl_token_metadata_account: metadata_account,
            deployer_ata,
            event_authority,
            program: program_id,
        }
        .to_account_metas(None),
        data: ix_data,
    };

    let accounts = vec![
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
        (token_mint_pda, Account::new(0, 0, &system_program::ID)),
        (token_manager_ata, Account::new(0, 0, &system_program::ID)),
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
        // Instructions sysvar
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
        // MPL Token Metadata program
        (
            mpl_token_metadata::programs::MPL_TOKEN_METADATA_ID,
            Account {
                lamports: 1,
                data: vec![],
                owner: solana_sdk::bpf_loader_upgradeable::id(),
                executable: true,
                rent_epoch: 0,
            },
        ),
        (metadata_account, Account::new(0, 0, &system_program::ID)),
        (deployer_ata, Account::new(0, 0, &system_program::ID)),
        // For event CPI
        (event_authority, event_authority_account),
        (program_id, program_account),
    ];

    let result = mollusk.process_instruction(&ix, &accounts);

    assert!(
        result.program_result.is_ok(),
        "Deploy interchain token instruction should succeed: {:?}",
        result.program_result
    );

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
