use crate::{
    instruction::GetTransaction,
    seed_prefixes::{
        INTERCHAIN_EXECUTABLE_TRANSACTION_PDA_SEED, PREFIX_CANONICAL_TOKEN_SALT,
        PREFIX_CUSTOM_TOKEN_SALT, PREFIX_INTERCHAIN_TOKEN_ID, PREFIX_INTERCHAIN_TOKEN_SALT,
    },
    InterchainTokenService, TokenManager, CHAIN_NAME_HASH,
};
use anchor_lang::{prelude::*, solana_program};
use relayer_discovery::structs::{
    RelayerAccount, RelayerData, RelayerInstruction, RelayerTransaction,
};

pub fn interchain_token_deployer_salt(deployer: &Pubkey, salt: &[u8; 32]) -> [u8; 32] {
    solana_program::keccak::hashv(&[
        PREFIX_INTERCHAIN_TOKEN_SALT,
        &CHAIN_NAME_HASH,
        deployer.as_ref(),
        salt,
    ])
    .to_bytes()
}

pub fn interchain_token_id_internal(salt: &[u8; 32]) -> [u8; 32] {
    solana_program::keccak::hashv(&[PREFIX_INTERCHAIN_TOKEN_ID, salt]).to_bytes()
}

pub fn interchain_token_id(deployer: &Pubkey, salt: &[u8; 32]) -> [u8; 32] {
    let deploy_salt = interchain_token_deployer_salt(deployer, salt);

    interchain_token_id_internal(&deploy_salt)
}

pub fn canonical_interchain_token_deploy_salt(mint: &Pubkey) -> [u8; 32] {
    solana_program::keccak::hashv(&[PREFIX_CANONICAL_TOKEN_SALT, &CHAIN_NAME_HASH, mint.as_ref()])
        .to_bytes()
}

pub fn canonical_interchain_token_id(mint: &Pubkey) -> [u8; 32] {
    let salt = canonical_interchain_token_deploy_salt(mint);

    interchain_token_id_internal(&salt)
}

pub fn linked_token_deployer_salt(deployer: &Pubkey, salt: &[u8; 32]) -> [u8; 32] {
    solana_program::keccak::hashv(&[
        PREFIX_CUSTOM_TOKEN_SALT,
        &CHAIN_NAME_HASH,
        deployer.as_ref(),
        salt,
    ])
    .to_bytes()
}

pub fn truncate_utf8(s: &mut String, max_bytes: usize) {
    if s.len() <= max_bytes {
        return;
    }
    let mut cut = max_bytes;
    while !s.is_char_boundary(cut) {
        cut -= 1;
    }
    s.truncate(cut);
}

pub fn find_interchain_executable_transaction_pda(interchain_executable: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[INTERCHAIN_EXECUTABLE_TRANSACTION_PDA_SEED],
        interchain_executable,
    )
}

pub fn relayer_transaction(
    token_id: Option<[u8; 32]>,
    destination_transaction: Option<Pubkey>,
) -> RelayerTransaction {
    let token_manager = match token_id {
        Some(token_id) => TokenManager::find_pda(token_id, InterchainTokenService::find_pda().0).0,
        None => crate::ID,
    };
    let destination_transaction = match destination_transaction {
        Some(destination_transaction) => destination_transaction,
        None => crate::ID,
    };
    RelayerTransaction::Discovery(RelayerInstruction {
        // We want the relayer to call this program.
        program_id: crate::ID,
        // No accounts are required for this.
        accounts: vec![
            RelayerAccount::Account {
                pubkey: token_manager,
                is_writable: false,
            },
            RelayerAccount::Account {
                pubkey: destination_transaction,
                is_writable: false,
            },
        ],
        // The data we need to find the final transaction.
        data: vec![
            // We can easily get the discriminaator thankfully. Note that we need `instruction::GetTransaction` and not `instructions::GetTransaction`.
            RelayerData::Bytes(Vec::from(GetTransaction::DISCRIMINATOR)),
            // We do not want to prefix the payload with the length as it is decoded into a struct as opposed to a `Vec<u8>`.
            RelayerData::Message,
            // The command id, which is the only thing required (alongside this crate's id) to derive all the accounts required by the gateway.
            RelayerData::Payload,
        ],
    })
}
