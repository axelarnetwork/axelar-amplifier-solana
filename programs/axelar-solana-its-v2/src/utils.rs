use crate::{
    seed_prefixes::{
        PREFIX_CANONICAL_TOKEN_SALT, PREFIX_CUSTOM_TOKEN_SALT, PREFIX_INTERCHAIN_TOKEN_ID,
        PREFIX_INTERCHAIN_TOKEN_SALT,
    },
    CHAIN_NAME_HASH,
};
use anchor_lang::{prelude::*, solana_program};

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
    solana_program::keccak::hashv(&[deployer.as_ref(), salt]).to_bytes()
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
