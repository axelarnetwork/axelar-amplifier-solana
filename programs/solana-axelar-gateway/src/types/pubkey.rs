use anchor_lang::prelude::*;

pub const ED25519_PUBKEY_LEN: usize = 32;
pub const SECP256K1_COMPRESSED_PUBKEY_LEN: usize = 33;
pub type Secp256k1Pubkey = [u8; SECP256K1_COMPRESSED_PUBKEY_LEN];
pub type Ed25519Pubkey = [u8; ED25519_PUBKEY_LEN];

#[derive(
    Clone,
    Copy,
    Ord,
    PartialOrd,
    PartialEq,
    Eq,
    Debug,
    udigest::Digestable,
    AnchorSerialize,
    AnchorDeserialize,
)]
pub enum PublicKey {
    Secp256k1(Secp256k1Pubkey),
    Ed25519(Ed25519Pubkey),
}
