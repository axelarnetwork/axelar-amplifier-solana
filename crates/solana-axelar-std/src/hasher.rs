//! Type definitions for the Merkle tree primitives used in this crate.

#[cfg(not(any(feature = "solana", feature = "sha3")))]
compile_error!("Either the `solana` or `sha3` feature must be enabled for solana-axelar-std");

use arrayref::mut_array_refs;
pub use rs_merkle::{MerkleProof, MerkleTree};
use udigest::encoding::EncodeValue;

/// Computes a Keccak256 hash of the input data.
///
/// Uses `solana-keccak-hasher` when the `solana` feature is enabled,
/// otherwise falls back to the `sha3` crate.
#[cfg(feature = "solana")]
#[inline]
fn keccak256(data: &[u8]) -> [u8; 32] {
    solana_keccak_hasher::hash(data).to_bytes()
}

/// Computes a Keccak256 hash of the input data.
///
/// Uses `solana-keccak-hasher` when the `solana` feature is enabled,
/// otherwise falls back to the `sha3` crate.
#[cfg(all(feature = "sha3", not(feature = "solana")))]
#[inline]
fn keccak256(data: &[u8]) -> [u8; 32] {
    use sha3::Digest;
    sha3::Keccak256::digest(data).into()
}

/// Computes a Keccak256 hash over multiple byte slices.
///
/// Uses `solana-keccak-hasher` when the `solana` feature is enabled,
/// otherwise falls back to the `sha3` crate.
#[cfg(feature = "solana")]
#[inline]
pub fn keccak256v(data: &[&[u8]]) -> [u8; 32] {
    solana_keccak_hasher::hashv(data).to_bytes()
}

/// Computes a Keccak256 hash over multiple byte slices.
///
/// Uses `solana-keccak-hasher` when the `solana` feature is enabled,
/// otherwise falls back to the `sha3` crate.
#[cfg(all(feature = "sha3", not(feature = "solana")))]
#[inline]
pub fn keccak256v(data: &[&[u8]]) -> [u8; 32] {
    use sha3::Digest;
    let mut hasher = sha3::Keccak256::new();
    for slice in data {
        hasher.update(slice);
    }
    hasher.finalize().into()
}

#[derive(Copy, Clone)]
pub struct Hasher;

impl rs_merkle::Hasher for Hasher {
    type Hash = [u8; 32];

    fn hash(data: &[u8]) -> Self::Hash {
        keccak256(data)
    }

    /// This implementation deviates from the default for several reasons:
    /// 1. It prefixes intermediate nodes before hashing to prevent second preimage
    ///    attacks. This distinguishes leaf nodes from intermediates, blocking
    ///    attempts to craft alternative trees with the same root hash using
    ///    malicious hashes.
    /// 2. If the left node doesn't have a sibling it is concatenated to itself and
    ///    then hashed instead of just being propagated to the next level.
    /// 3. It uses arrays instead of vectors to avoid heap allocations.
    fn concat_and_hash(left: &Self::Hash, right: Option<&Self::Hash>) -> Self::Hash {
        let mut concatenated: [u8; 65] = [0; 65];
        let (prefix, left_node, right_node) = mut_array_refs![&mut concatenated, 1, 32, 32];
        prefix[0] = 1;
        left_node.copy_from_slice(left);
        right_node.copy_from_slice(right.unwrap_or(left));
        keccak256(&concatenated)
    }
}

pub(crate) struct VecBuf(pub(crate) Vec<u8>);

impl udigest::encoding::Buffer for VecBuf {
    fn write(&mut self, bytes: &[u8]) {
        self.0.extend_from_slice(bytes);
    }
}

/// Trait for hashing leaves within a Merkle tree, implemented by types that can
/// be digested.
pub trait LeafHash: udigest::Digestable {
    /// Returns a hashed representation of the implementing type.
    fn hash(&self) -> [u8; 32] {
        let mut buffer = VecBuf(vec![]);
        self.unambiguously_encode(EncodeValue::new(&mut buffer));
        keccak256(&buffer.0)
    }
}
