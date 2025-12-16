use crate::{
    seed_prefixes::{
        PREFIX_CANONICAL_TOKEN_SALT, PREFIX_CUSTOM_TOKEN_SALT, PREFIX_INTERCHAIN_TOKEN_ID,
        PREFIX_INTERCHAIN_TOKEN_SALT,
    },
    CHAIN_NAME_HASH,
};
use anchor_lang::prelude::*;

pub fn interchain_token_deployer_salt(deployer: &Pubkey, salt: &[u8; 32]) -> [u8; 32] {
    solana_keccak_hasher::hashv(&[
        PREFIX_INTERCHAIN_TOKEN_SALT,
        &CHAIN_NAME_HASH,
        deployer.as_ref(),
        salt,
    ])
    .0
}

pub fn interchain_token_id_internal(salt: &[u8; 32]) -> [u8; 32] {
    solana_keccak_hasher::hashv(&[PREFIX_INTERCHAIN_TOKEN_ID, salt]).0
}

pub fn interchain_token_id(deployer: &Pubkey, salt: &[u8; 32]) -> [u8; 32] {
    let deploy_salt = interchain_token_deployer_salt(deployer, salt);

    interchain_token_id_internal(&deploy_salt)
}

pub fn canonical_interchain_token_deploy_salt(mint: &Pubkey) -> [u8; 32] {
    solana_keccak_hasher::hashv(&[PREFIX_CANONICAL_TOKEN_SALT, &CHAIN_NAME_HASH, mint.as_ref()]).0
}

pub fn canonical_interchain_token_id(mint: &Pubkey) -> [u8; 32] {
    let salt = canonical_interchain_token_deploy_salt(mint);

    interchain_token_id_internal(&salt)
}

pub fn linked_token_deployer_salt(deployer: &Pubkey, salt: &[u8; 32]) -> [u8; 32] {
    solana_keccak_hasher::hashv(&[
        PREFIX_CUSTOM_TOKEN_SALT,
        &CHAIN_NAME_HASH,
        deployer.as_ref(),
        salt,
    ])
    .0
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
