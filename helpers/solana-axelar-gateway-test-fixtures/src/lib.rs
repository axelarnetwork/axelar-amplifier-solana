#![allow(clippy::too_many_arguments)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::indexing_slicing)]

use anchor_lang::{prelude::UpgradeableLoaderState, InstructionData, ToAccountMetas};
use anchor_lang::{AccountDeserialize, AnchorDeserialize};
use libsecp256k1::SecretKey;
use mollusk_svm::{result::InstructionResult, Mollusk};
use solana_axelar_gateway::{
    state::config::{InitialVerifierSet, InitializeConfigParams},
    CallContractSigner, ID as GATEWAY_PROGRAM_ID,
};
use solana_axelar_gateway::{
    GatewayConfig, IncomingMessage, SignatureVerificationSessionData, VerifierSetTracker,
};
use solana_axelar_std::execute_data::{
    encode, hash_payload, prefixed_message_hash_payload_type, ExecuteData,
};
use solana_axelar_std::hasher::Hasher;
use solana_axelar_std::{
    CrossChainId, MerkleTree, MerklizedMessage, Message, Messages, Payload, PayloadType, Signature,
    SigningVerifierSetInfo, VerifierSet, U256,
};
use solana_axelar_std::{PublicKey, VerifierSetLeaf};
use solana_sdk::{
    account::Account,
    instruction::{AccountMeta, Instruction},
    native_token::LAMPORTS_PER_SOL,
    pubkey::Pubkey,
};
use solana_sdk_ids::system_program::ID as SYSTEM_PROGRAM_ID;
use std::collections::BTreeMap;

pub struct TestSetup {
    pub mollusk: Mollusk,
    pub payer: Pubkey,
    pub upgrade_authority: Pubkey,
    pub operator: Pubkey,
    pub gateway_root_pda: Pubkey,
    pub gateway_bump: u8,
    pub program_data_pda: Pubkey,
    pub verifier_set_tracker_pda: Pubkey,
    pub verifier_bump: u8,
    pub verifier_set_hash: [u8; 32],
    pub domain_separator: [u8; 32],
    pub minimum_rotation_delay: u64,
    pub epoch: U256,
    pub previous_verifier_retention: U256,
    pub verifier_set: VerifierSet,
    pub gateway_caller_pda: Option<Pubkey>,
    pub gateway_caller_bump: Option<u8>,
    pub event_authority_pda: Option<Pubkey>,
    pub event_authority_bump: Option<u8>,
}

pub fn mock_setup_test(gateway_caller_program_id: Option<Pubkey>) -> TestSetup {
    let mollusk = Mollusk::new(
        &GATEWAY_PROGRAM_ID,
        "../../target/deploy/solana_axelar_gateway",
    );

    let payer = Pubkey::new_unique();
    let upgrade_authority = Pubkey::new_unique();
    let operator = Pubkey::new_unique();

    let domain_separator = [1u8; 32];
    let verifier_set_hash = [2u8; 32];
    let minimum_rotation_delay = 3600;
    let epoch = U256::from(1u64);
    let previous_verifier_retention = U256::from(5u64);

    // Create a mock verifier set
    let dummy_pubkey = PublicKey([1u8; 33]);
    let mut signers = BTreeMap::new();
    signers.insert(dummy_pubkey, 100u128);
    let verifier_set = VerifierSet {
        nonce: 0,
        signers,
        quorum: 100,
    };

    // Derive PDAs
    let (gateway_root_pda, gateway_bump) = GatewayConfig::find_pda();

    let (program_data_pda, _) = Pubkey::find_program_address(
        &[GATEWAY_PROGRAM_ID.as_ref()],
        &solana_sdk_ids::bpf_loader_upgradeable::id(),
    );

    let (verifier_set_tracker_pda, verifier_bump) =
        VerifierSetTracker::find_pda(&verifier_set_hash);

    match gateway_caller_program_id {
        Some(program_id) => {
            // Derive PDAs specific to memo program
            let (gateway_caller_pda, gateway_caller_bump) =
                CallContractSigner::find_pda(&program_id);

            let (event_authority_pda, event_authority_bump) =
                Pubkey::find_program_address(&[b"__event_authority"], &GATEWAY_PROGRAM_ID);

            TestSetup {
                mollusk,
                payer,
                upgrade_authority,
                operator,
                gateway_root_pda,
                gateway_bump,
                program_data_pda,
                verifier_set_tracker_pda,
                verifier_bump,
                verifier_set_hash,
                domain_separator,
                minimum_rotation_delay,
                epoch,
                previous_verifier_retention,
                verifier_set: verifier_set.clone(),
                gateway_caller_pda: Some(gateway_caller_pda),
                gateway_caller_bump: Some(gateway_caller_bump),
                event_authority_pda: Some(event_authority_pda),
                event_authority_bump: Some(event_authority_bump),
            }
        }
        None => TestSetup {
            mollusk,
            payer,
            upgrade_authority,
            operator,
            gateway_root_pda,
            gateway_bump,
            program_data_pda,
            verifier_set_tracker_pda,
            verifier_bump,
            verifier_set_hash,
            domain_separator,
            minimum_rotation_delay,
            epoch,
            previous_verifier_retention,
            verifier_set,
            gateway_caller_pda: None,
            gateway_caller_bump: None,
            event_authority_pda: None,
            event_authority_bump: None,
        },
    }
}

pub fn setup_test_with_real_signers() -> (TestSetup, SecretKey, SecretKey) {
    let mollusk = Mollusk::new(
        &GATEWAY_PROGRAM_ID,
        "../../target/deploy/solana_axelar_gateway",
    );

    let payer = Pubkey::new_unique();
    let upgrade_authority = Pubkey::new_unique();
    let operator = Pubkey::new_unique();

    // Step 1: Create REAL signers first
    let (secret_key_1, public_key_1) = generate_random_signer();
    let (secret_key_2, public_key_2) = generate_random_signer();

    let epoch = U256::from(1u64);
    let previous_verifier_retention = U256::from(5u64);
    let domain_separator = [2u8; 32];
    let minimum_rotation_delay = 3600;

    let (verifier_set_hash, verifier_set) =
        create_merklized_verifier_set_from_keypairs(domain_separator, public_key_1, public_key_2);

    // Step 4: Derive PDAs with the REAL verifier set hash
    let (gateway_root_pda, gateway_bump) = GatewayConfig::find_pda();

    let (program_data_pda, _) = Pubkey::find_program_address(
        &[GATEWAY_PROGRAM_ID.as_ref()],
        &solana_sdk_ids::bpf_loader_upgradeable::id(),
    );

    let (verifier_set_tracker_pda, verifier_bump) =
        VerifierSetTracker::find_pda(&verifier_set_hash);

    let setup = TestSetup {
        mollusk,
        payer,
        upgrade_authority,
        operator,
        gateway_root_pda,
        gateway_bump,
        program_data_pda,
        verifier_set_tracker_pda,
        verifier_bump,
        verifier_set_hash,
        domain_separator,
        minimum_rotation_delay,
        epoch,
        previous_verifier_retention,
        verifier_set,
        gateway_caller_pda: None,
        gateway_caller_bump: None,
        event_authority_pda: None,
        event_authority_bump: None,
    };

    (setup, secret_key_1, secret_key_2)
}

pub fn initialize_gateway(setup: &TestSetup) -> InstructionResult {
    let params = InitializeConfigParams {
        domain_separator: setup.domain_separator,
        initial_verifier_set: InitialVerifierSet {
            hash: setup.verifier_set_hash,
            pda: setup.verifier_set_tracker_pda,
        },
        minimum_rotation_delay: setup.minimum_rotation_delay,
        operator: setup.operator,
        previous_verifier_retention: setup.previous_verifier_retention,
    };

    let instruction_data = solana_axelar_gateway::instruction::InitializeConfig { params }.data();

    let program_data_state = UpgradeableLoaderState::ProgramData {
        slot: 0,
        upgrade_authority_address: Some(setup.upgrade_authority),
    };

    let serialized_program_data = bincode::serialize(&program_data_state).unwrap();

    let accounts = vec![
        (
            setup.payer,
            Account {
                lamports: LAMPORTS_PER_SOL,
                data: vec![],
                owner: SYSTEM_PROGRAM_ID,
                executable: false,
                rent_epoch: 0,
            },
        ),
        (
            setup.upgrade_authority,
            Account {
                lamports: LAMPORTS_PER_SOL,
                data: vec![],
                owner: SYSTEM_PROGRAM_ID,
                executable: false,
                rent_epoch: 0,
            },
        ),
        (
            setup.program_data_pda,
            Account {
                lamports: LAMPORTS_PER_SOL,
                data: serialized_program_data,
                owner: solana_sdk_ids::bpf_loader_upgradeable::id(),
                executable: false,
                rent_epoch: 0,
            },
        ),
        (
            setup.gateway_root_pda,
            Account {
                lamports: 0,
                data: vec![],
                owner: SYSTEM_PROGRAM_ID,
                executable: false,
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
            setup.verifier_set_tracker_pda,
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
        program_id: GATEWAY_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(setup.payer, true),
            AccountMeta::new_readonly(setup.upgrade_authority, true),
            AccountMeta::new_readonly(setup.program_data_pda, false),
            AccountMeta::new(setup.gateway_root_pda, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
            AccountMeta::new(setup.verifier_set_tracker_pda, false),
        ],
        data: instruction_data,
    };

    setup.mollusk.process_instruction(&instruction, &accounts)
}

pub fn generate_random_signer() -> (SecretKey, [u8; 33]) {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let secret_key_bytes: [u8; 32] = rng.gen();
    let secret_key = libsecp256k1::SecretKey::parse(&secret_key_bytes).unwrap();
    let public_key = libsecp256k1::PublicKey::from_secret_key(&secret_key);
    let compressed_pubkey = public_key.serialize_compressed();

    (secret_key, compressed_pubkey)
}

pub fn create_test_message(
    source_chain: &str,
    message_id: &str,
    destination_address: &str,
    payload_hash: [u8; 32],
) -> Message {
    Message {
        cc_id: CrossChainId {
            chain: source_chain.to_owned(),
            id: message_id.to_owned(),
        },
        source_address: "0xSourceAddress".to_owned(),
        destination_chain: "solana".to_owned(),
        destination_address: destination_address.to_owned(),
        payload_hash,
    }
}

pub fn initialize_payload_verification_session(
    setup: &TestSetup,
    gateway_account: Account,
    verifier_set_tracker_account: Account,
    payload_merkle_root: [u8; 32],
    payload_type: PayloadType,
) -> (InstructionResult, Pubkey) {
    let signing_verifier_set_hash = setup.verifier_set_hash;

    let (verification_session_pda, _) = SignatureVerificationSessionData::find_pda(
        &payload_merkle_root,
        payload_type,
        &signing_verifier_set_hash,
    );

    let instruction_data =
        solana_axelar_gateway::instruction::InitializePayloadVerificationSession {
            merkle_root: payload_merkle_root,
            payload_type,
        }
        .data();

    let accounts = vec![
        (
            setup.payer,
            Account {
                lamports: LAMPORTS_PER_SOL,
                data: vec![],
                owner: SYSTEM_PROGRAM_ID,
                executable: false,
                rent_epoch: 0,
            },
        ),
        (setup.gateway_root_pda, gateway_account),
        (
            verification_session_pda,
            Account {
                lamports: 0,
                data: vec![],
                owner: SYSTEM_PROGRAM_ID,
                executable: false,
                rent_epoch: 0,
            },
        ),
        (setup.verifier_set_tracker_pda, verifier_set_tracker_account),
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
    ];

    let instruction = Instruction {
        program_id: GATEWAY_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(setup.payer, true),
            AccountMeta::new_readonly(setup.gateway_root_pda, false),
            AccountMeta::new(verification_session_pda, false),
            AccountMeta::new_readonly(setup.verifier_set_tracker_pda, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
        ],
        data: instruction_data,
    };

    (
        setup.mollusk.process_instruction(&instruction, &accounts),
        verification_session_pda,
    )
}

pub fn create_verifier_info(
    secret_key: &SecretKey,
    payload_merkle_root: [u8; 32],
    verifier_leaf: &VerifierSetLeaf,
    position: usize,
    verifier_merkle_tree: &MerkleTree,
    payload_type: PayloadType,
) -> SigningVerifierSetInfo {
    let hashed_message = prefixed_message_hash_payload_type(payload_type, &payload_merkle_root);

    let message = libsecp256k1::Message::parse(&hashed_message);
    let signature = sign_message(&message, secret_key);

    let merkle_proof = verifier_merkle_tree.proof(&[position]);
    let merkle_proof_bytes = merkle_proof.to_bytes();

    SigningVerifierSetInfo {
        signature,
        leaf: *verifier_leaf,
        merkle_proof: merkle_proof_bytes,
        payload_type,
    }
}

pub fn call_contract_helper(
    setup: &TestSetup,
    gateway_account: Account,
    memo_program_id: Pubkey,
) -> InstructionResult {
    let signing_pda = setup.gateway_caller_pda.unwrap();
    let (event_authority_pda, _) =
        Pubkey::find_program_address(&[b"__event_authority"], &GATEWAY_PROGRAM_ID);

    let mut accounts = vec![
        (
            memo_program_id,
            Account {
                lamports: 1,
                data: vec![],
                owner: solana_sdk::native_loader::id(),
                executable: true,
                rent_epoch: 0,
            },
        ),
        (
            signing_pda,
            Account {
                lamports: 0,
                data: vec![],
                owner: memo_program_id,
                executable: false,
                rent_epoch: 0,
            },
        ),
        (
            event_authority_pda,
            Account {
                lamports: 1,
                data: vec![],
                owner: GATEWAY_PROGRAM_ID,
                executable: false,
                rent_epoch: 0,
            },
        ),
        (
            GATEWAY_PROGRAM_ID,
            Account {
                lamports: 1,
                data: vec![],
                owner: solana_sdk_ids::bpf_loader_upgradeable::id(),
                executable: true,
                rent_epoch: 0,
            },
        ),
    ];

    accounts.push((setup.gateway_root_pda, gateway_account));

    let destination_chain = "ethereum".to_owned();
    let destination_contract_address = "0xdeadbeef".to_owned();
    let payload = b"memo test".to_vec();

    let signing_pda_bump = setup.gateway_caller_bump.unwrap();

    let ix_data = solana_axelar_gateway::instruction::CallContract {
        destination_chain,
        destination_contract_address,
        payload,
        signing_pda_bump,
    }
    .data();

    // Full account metas (must include event_authority + program)
    let ix = Instruction {
        program_id: GATEWAY_PROGRAM_ID,
        accounts: solana_axelar_gateway::accounts::CallContract {
            caller: memo_program_id,
            signing_pda: Some(signing_pda),
            gateway_root_pda: setup.gateway_root_pda,
            event_authority: event_authority_pda,
            program: GATEWAY_PROGRAM_ID,
        }
        .to_account_metas(None),
        data: ix_data,
    };

    setup.mollusk.process_instruction(&ix, &accounts)
}

pub fn verify_signature_helper(
    setup: &TestSetup,
    payload_merkle_root: [u8; 32],
    verifier_info: SigningVerifierSetInfo,
    verification_session: (Pubkey, Account),
    gateway_account: Account,
    verifier_set_tracker: (Pubkey, Account),
) -> InstructionResult {
    let instruction_data = solana_axelar_gateway::instruction::VerifySignature {
        payload_merkle_root,
        verifier_info,
    }
    .data();

    let accounts = vec![
        (setup.gateway_root_pda, gateway_account),
        (verification_session.0, verification_session.1),
        (verifier_set_tracker.0, verifier_set_tracker.1),
    ];

    let instruction = Instruction {
        program_id: GATEWAY_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new_readonly(setup.gateway_root_pda, false),
            AccountMeta::new(verification_session.0, false),
            AccountMeta::new_readonly(verifier_set_tracker.0, false),
        ],
        data: instruction_data,
    };

    // Execute the verify_signature instruction
    setup.mollusk.process_instruction(&instruction, &accounts)
}

pub fn rotate_signers_helper(
    setup: &TestSetup,
    new_verifier_set_hash: [u8; 32],
    verification_session: (Pubkey, Account),
    gateway_account: Account,
    verifier_set_tracker_account: Account,
) -> InstructionResult {
    let instruction_data = solana_axelar_gateway::instruction::RotateSigners {
        new_verifier_set_merkle_root: new_verifier_set_hash,
    }
    .data();

    let (new_verifier_set_tracker_pda, _) = VerifierSetTracker::find_pda(&new_verifier_set_hash);

    let (event_authority_pda, _) =
        Pubkey::find_program_address(&[b"__event_authority"], &GATEWAY_PROGRAM_ID);

    let accounts = vec![
        (setup.gateway_root_pda, gateway_account),
        (verification_session.0, verification_session.1),
        (setup.verifier_set_tracker_pda, verifier_set_tracker_account),
        (
            new_verifier_set_tracker_pda,
            Account {
                lamports: 0,
                data: vec![],
                owner: SYSTEM_PROGRAM_ID,
                executable: false,
                rent_epoch: 0,
            },
        ),
        (
            setup.payer,
            Account {
                lamports: LAMPORTS_PER_SOL,
                data: vec![],
                owner: SYSTEM_PROGRAM_ID,
                executable: false,
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
            setup.operator,
            Account {
                lamports: LAMPORTS_PER_SOL,
                data: vec![],
                owner: SYSTEM_PROGRAM_ID,
                executable: false,
                rent_epoch: 0,
            },
        ),
        // for cpi events
        (
            event_authority_pda,
            Account {
                lamports: LAMPORTS_PER_SOL,
                data: vec![],
                owner: GATEWAY_PROGRAM_ID,
                executable: false,
                rent_epoch: 0,
            },
        ),
        (
            GATEWAY_PROGRAM_ID,
            Account {
                lamports: LAMPORTS_PER_SOL,
                data: vec![],
                owner: solana_sdk_ids::bpf_loader_upgradeable::id(),
                executable: true,
                rent_epoch: 0,
            },
        ),
    ];

    let instruction = Instruction {
        program_id: GATEWAY_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(setup.gateway_root_pda, false),
            AccountMeta::new_readonly(verification_session.0, false),
            AccountMeta::new_readonly(setup.verifier_set_tracker_pda, false),
            AccountMeta::new(new_verifier_set_tracker_pda, false),
            AccountMeta::new(setup.payer, true),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
            // optional operator
            AccountMeta::new(setup.operator, true),
            // for event cpi
            AccountMeta::new_readonly(event_authority_pda, false),
            AccountMeta::new_readonly(GATEWAY_PROGRAM_ID, false),
        ],
        data: instruction_data,
    };

    setup.mollusk.process_instruction(&instruction, &accounts)
}

pub fn transfer_operatorship_helper(
    setup: &TestSetup,
    gateway_account: Account,
    program_data_account: Account,
    new_operator: Pubkey,
) -> InstructionResult {
    let instruction_data = solana_axelar_gateway::instruction::TransferOperatorship {}.data();

    let (event_authority_pda, _) =
        Pubkey::find_program_address(&[b"__event_authority"], &GATEWAY_PROGRAM_ID);

    let accounts = vec![
        (setup.gateway_root_pda, gateway_account),
        (setup.program_data_pda, program_data_account),
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
        (
            new_operator,
            Account {
                lamports: 0,
                data: vec![],
                owner: SYSTEM_PROGRAM_ID,
                executable: false,
                rent_epoch: 0,
            },
        ),
        // for cpi events
        (
            event_authority_pda,
            Account {
                lamports: LAMPORTS_PER_SOL,
                data: vec![],
                owner: GATEWAY_PROGRAM_ID,
                executable: false,
                rent_epoch: 0,
            },
        ),
        (
            GATEWAY_PROGRAM_ID,
            Account {
                lamports: LAMPORTS_PER_SOL,
                data: vec![],
                owner: solana_sdk_ids::bpf_loader_upgradeable::id(),
                executable: true,
                rent_epoch: 0,
            },
        ),
    ];

    let instruction = Instruction {
        program_id: GATEWAY_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(setup.gateway_root_pda, false),
            AccountMeta::new_readonly(setup.operator, true),
            AccountMeta::new_readonly(setup.program_data_pda, false),
            AccountMeta::new_readonly(new_operator, false),
            // for CPI events
            AccountMeta::new_readonly(event_authority_pda, false),
            AccountMeta::new_readonly(GATEWAY_PROGRAM_ID, false),
        ],
        data: instruction_data,
    };

    // Execute the instruction
    setup.mollusk.process_instruction(&instruction, &accounts)
}

pub fn approve_message_helper_from_merklized(
    setup: &TestSetup,
    merklized_message: &MerklizedMessage,
    payload_merkle_root: [u8; 32],
    verification_session: (Pubkey, Account),
    gateway_account: Account,
) -> (InstructionResult, Pubkey) {
    let command_id = merklized_message.leaf.message.command_id();

    let (incoming_message_pda, _) = IncomingMessage::find_pda(&command_id);

    let (event_authority_pda, _) =
        Pubkey::find_program_address(&[b"__event_authority"], &GATEWAY_PROGRAM_ID);

    let approve_instruction_data = solana_axelar_gateway::instruction::ApproveMessage {
        merklized_message: merklized_message.clone(),
        payload_merkle_root,
    }
    .data();

    let approve_accounts = vec![
        (setup.gateway_root_pda, gateway_account),
        (
            setup.payer,
            Account {
                lamports: LAMPORTS_PER_SOL,
                data: vec![],
                owner: SYSTEM_PROGRAM_ID,
                executable: false,
                rent_epoch: 0,
            },
        ),
        (verification_session.0, verification_session.1),
        (
            incoming_message_pda,
            Account {
                lamports: 0,
                data: vec![],
                owner: SYSTEM_PROGRAM_ID,
                executable: false,
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
            event_authority_pda,
            Account {
                lamports: LAMPORTS_PER_SOL,
                data: vec![],
                owner: GATEWAY_PROGRAM_ID,
                executable: false,
                rent_epoch: 0,
            },
        ),
        (
            GATEWAY_PROGRAM_ID,
            Account {
                lamports: LAMPORTS_PER_SOL,
                data: vec![],
                owner: solana_sdk_ids::bpf_loader_upgradeable::id(),
                executable: true,
                rent_epoch: 0,
            },
        ),
    ];

    let approve_instruction = Instruction {
        program_id: GATEWAY_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new_readonly(setup.gateway_root_pda, false),
            AccountMeta::new(setup.payer, true),
            AccountMeta::new_readonly(verification_session.0, false),
            AccountMeta::new(incoming_message_pda, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
            AccountMeta::new_readonly(event_authority_pda, false),
            AccountMeta::new_readonly(GATEWAY_PROGRAM_ID, false),
        ],
        data: approve_instruction_data,
    };

    (
        setup
            .mollusk
            .process_instruction(&approve_instruction, &approve_accounts),
        incoming_message_pda,
    )
}

pub fn default_messages() -> Vec<Message> {
    vec![
        create_test_message(
            "ethereum",
            "msg_1",
            "DNHKNbf4JWJNnquuWJuNUSFGsXbDYs1sPR1ZvVhah827",
            [1u8; 32],
        ),
        create_test_message(
            "ethereum",
            "msg_2",
            "8q49wyQjNrSEZf5A8h6jR7dwLnDxdnURftv89FWLWMGK",
            [2u8; 32],
        ),
    ]
}

pub fn fake_messages() -> Vec<Message> {
    vec![
        create_test_message(
            "ethereum",
            "fake msg_1",
            "DNHKNbf4JWJNnquuWJuNUSFGsXbDYs1sPR1ZvVhah827",
            [1u8; 32],
        ),
        create_test_message(
            "ethereum",
            "fake msg_2",
            "8q49wyQjNrSEZf5A8h6jR7dwLnDxdnURftv89FWLWMGK",
            [2u8; 32],
        ),
    ]
}

#[allow(clippy::panic)]
pub fn create_merklized_messages_from_std(
    domain_separator: [u8; 32],
    messages: &[Message],
) -> (Vec<MerklizedMessage>, [u8; 32]) {
    // Note: create minimal verifier set with one dummy signer (we only need the payload part)
    let dummy_pubkey = PublicKey([1u8; 33]);
    let mut signers = BTreeMap::new();
    signers.insert(dummy_pubkey, 1u128);

    let verifier_set = VerifierSet {
        nonce: 0,
        signers,
        quorum: 1,
    };
    let signatures = BTreeMap::new();

    let payload = Payload::Messages(Messages(messages.to_vec()));

    let encoded = encode(&verifier_set, &signatures, domain_separator, payload).unwrap();
    let execute_data = solana_axelar_std::execute_data::ExecuteData::try_from_slice(&encoded)
        .map_err(|_| solana_axelar_std::EncodingError::CannotMerklizeEmptyMessageSet)
        .unwrap();

    if let solana_axelar_std::execute_data::MerklizedPayload::NewMessages { messages } =
        execute_data.payload_items
    {
        (messages, execute_data.payload_merkle_root)
    } else {
        panic!("Expected NewMessages payload")
    }
}

fn sign_message(message: &libsecp256k1::Message, secret_key: &SecretKey) -> Signature {
    let (sig, recovery_id) = libsecp256k1::sign(message, secret_key);
    let mut bytes = [0u8; 65];
    bytes[..64].copy_from_slice(&sig.serialize());
    bytes[64] = recovery_id.serialize();
    Signature(bytes)
}

pub fn create_execute_data_with_signatures(
    domain_separator: [u8; 32],
    secret_key_1: &SecretKey,
    secret_key_2: &SecretKey,
    payload_to_be_signed: Payload,
    current_verifier_set: VerifierSet,
) -> ExecuteData {
    // Extract public keys from secret keys
    let public_key_1 = libsecp256k1::PublicKey::from_secret_key(secret_key_1);
    let public_key_2 = libsecp256k1::PublicKey::from_secret_key(secret_key_2);
    let pubkey_1 = PublicKey(public_key_1.serialize_compressed());
    let pubkey_2 = PublicKey(public_key_2.serialize_compressed());

    let payload_merkle_root =
        hash_payload::<Hasher>(&domain_separator, payload_to_be_signed.clone()).unwrap();

    let payload_type = match payload_to_be_signed {
        Payload::Messages(_) => PayloadType::ApproveMessages,
        Payload::NewVerifierSet(_) => PayloadType::RotateSigners,
    };

    let hashed_message = prefixed_message_hash_payload_type(payload_type, &payload_merkle_root);
    let message = libsecp256k1::Message::parse(&hashed_message);

    // Create signatures for both signers
    let signature1 = sign_message(&message, secret_key_1);
    let signature2 = sign_message(&message, secret_key_2);

    let mut signatures = BTreeMap::new();
    signatures.insert(pubkey_1, signature1);
    signatures.insert(pubkey_2, signature2);

    let encoded = encode(
        &current_verifier_set,
        &signatures,
        domain_separator,
        payload_to_be_signed,
    )
    .unwrap();

    ExecuteData::try_from_slice(&encoded).unwrap()
}

pub fn create_signing_verifier_set_leaves(
    domain_separator: [u8; 32],
    secret_key_1: &SecretKey,
    secret_key_2: &SecretKey,
    payload_to_be_signed: Payload,
    current_verifier_set: VerifierSet,
) -> Vec<SigningVerifierSetInfo> {
    let execute_data = create_execute_data_with_signatures(
        domain_separator,
        secret_key_1,
        secret_key_2,
        payload_to_be_signed,
        current_verifier_set,
    );

    execute_data.signing_verifier_set_leaves
}

pub fn create_merklized_verifier_set_from_keypairs(
    domain_separator: [u8; 32],
    public_key_1: [u8; 33],
    public_key_2: [u8; 33],
) -> ([u8; 32], VerifierSet) {
    let pubkey_1 = PublicKey(public_key_1);
    let pubkey_2 = PublicKey(public_key_2);

    // Create the new verifier set with the two real signers
    let mut signers = BTreeMap::new();
    signers.insert(pubkey_1, 50u128);
    signers.insert(pubkey_2, 50u128);

    let new_verifier_set = VerifierSet {
        nonce: 1,
        signers,
        quorum: 100,
    };

    // Create signatures map (empty for this use case)
    let signatures = BTreeMap::new();

    let payload = Payload::NewVerifierSet(new_verifier_set.clone());

    let encoded = encode(&new_verifier_set, &signatures, domain_separator, payload).unwrap();
    let execute_data = solana_axelar_std::execute_data::ExecuteData::try_from_slice(&encoded)
        .map_err(|_| solana_axelar_std::EncodingError::CannotMerklizeEmptyVerifierSet)
        .unwrap();

    (
        execute_data.signing_verifier_set_merkle_root,
        new_verifier_set,
    )
}

pub fn approve_message_helper(
    setup: &TestSetup,
    messages: &[Message],
    verification_session: (Pubkey, Account),
    gateway_account: Account,
    position: usize,
) -> (InstructionResult, Pubkey) {
    // Use the new std-based approach
    let (merklized_messages, payload_merkle_root) =
        create_merklized_messages_from_std(setup.domain_separator, messages);

    let merklized_message = &merklized_messages[position];

    approve_message_helper_from_merklized(
        setup,
        merklized_message,
        payload_merkle_root,
        verification_session,
        gateway_account,
    )
}

pub fn approve_messages_on_gateway(
    setup: &TestSetup,
    messages: Vec<Message>,
    gateway_account: Account,
    verifier_set_tracker_account: Account,
    secret_key_1: &SecretKey,
    secret_key_2: &SecretKey,
) -> Vec<(IncomingMessage, Pubkey, Vec<u8>)> {
    // Create messages and payload merkle root using std crate
    let (merklized_messages, payload_merkle_root) =
        create_merklized_messages_from_std(setup.domain_separator, &messages);
    let payload_type = PayloadType::ApproveMessages;

    let (session_result, verification_session_pda) = initialize_payload_verification_session(
        setup,
        gateway_account.clone(),
        verifier_set_tracker_account.clone(),
        payload_merkle_root,
        payload_type,
    );
    assert!(
        !session_result.program_result.is_err(),
        "Failed to initialize verification session"
    );

    let verification_session_account = session_result
        .get_account(&verification_session_pda)
        .unwrap()
        .clone();

    // Create signing verifier set leaves using the new approach
    let payload_to_be_signed = Payload::Messages(Messages(messages.clone()));
    let signing_verifier_set_leaves = create_signing_verifier_set_leaves(
        setup.domain_separator,
        secret_key_1,
        secret_key_2,
        payload_to_be_signed,
        setup.verifier_set.clone(),
    );

    let verifier_info_1 = signing_verifier_set_leaves[0].clone();

    let verify_result_1 = verify_signature_helper(
        setup,
        payload_merkle_root,
        verifier_info_1,
        (
            verification_session_pda,
            verification_session_account.clone(),
        ),
        gateway_account.clone(),
        (
            setup.verifier_set_tracker_pda,
            verifier_set_tracker_account.clone(),
        ),
    );

    let updated_verification_account_after_first = verify_result_1
        .get_account(&verification_session_pda)
        .unwrap()
        .clone();

    let verifier_info_2 = signing_verifier_set_leaves[1].clone();

    let verify_result_2 = verify_signature_helper(
        setup,
        payload_merkle_root,
        verifier_info_2,
        (
            verification_session_pda,
            updated_verification_account_after_first,
        ),
        gateway_account,
        (setup.verifier_set_tracker_pda, verifier_set_tracker_account),
    );

    let final_gateway_account = verify_result_2
        .get_account(&setup.gateway_root_pda)
        .unwrap()
        .clone();

    let final_verification_session_account = verify_result_2
        .get_account(&verification_session_pda)
        .unwrap()
        .clone();

    let mut incoming_messages = Vec::new();

    // Approve all messages using the new approach
    for merklized_message in merklized_messages.iter().take(messages.len()) {
        let (approve_result, incoming_message_pda) = approve_message_helper_from_merklized(
            setup,
            merklized_message,
            payload_merkle_root,
            (
                verification_session_pda,
                final_verification_session_account.clone(),
            ),
            final_gateway_account.clone(),
        );

        let incoming_message_account = approve_result
            .get_account(&incoming_message_pda)
            .unwrap()
            .clone();

        // sanity check
        let incoming_message =
            IncomingMessage::try_deserialize(&mut incoming_message_account.data.as_slice())
                .unwrap();

        incoming_messages.push((
            incoming_message,
            incoming_message_pda,
            incoming_message_account.data,
        ));
    }

    incoming_messages
}
