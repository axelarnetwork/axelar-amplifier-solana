//! # Pubkey and Signature Types
//!
//! This module defines essential cryptographic types and constants used for
//! handling public keys and signatures within the system. It supports multiple
//! cryptographic algorithms, including Secp256k1 and Ed25519, providing a
//! unified interface for public key and signature management.
use borsh::{BorshDeserialize, BorshSerialize};
use udigest::Digestable;

//
// Pubkey
//

/// The length of a compressed Secp256k1 public key in bytes.
pub const SECP256K1_COMPRESSED_PUBKEY_LEN: usize = 33;

/// Type alias for a compressed Secp256k1 public key.
pub type Secp256k1Pubkey = [u8; SECP256K1_COMPRESSED_PUBKEY_LEN];

/// Represents a public key using supported cryptographic algorithms.
#[derive(
    Clone, Copy, Ord, PartialOrd, PartialEq, Eq, Hash, Digestable, BorshSerialize, BorshDeserialize,
)]
pub struct PublicKey(pub Secp256k1Pubkey);

#[allow(clippy::min_ident_chars)]
impl core::fmt::Debug for PublicKey {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let hex = hex::encode(self.0);
        f.write_str(hex.as_str())
    }
}

//
// Signature
//

/// The length of a recoverable ECDSA signature in bytes.
pub const ECDSA_RECOVERABLE_SIGNATURE_LEN: usize = 65;

/// Type alias for a recoverable ECDSA signature.
pub type EcdsaRecoverableSignature = [u8; ECDSA_RECOVERABLE_SIGNATURE_LEN];

/// Represents a digital signature using supported cryptographic algorithms.
#[derive(Eq, PartialEq, Clone, Copy, BorshDeserialize, BorshSerialize)]
pub struct Signature(pub EcdsaRecoverableSignature);

#[allow(clippy::min_ident_chars)]
impl core::fmt::Debug for Signature {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "EcdsaRecoverable({})", hex::encode(self.0))
    }
}
