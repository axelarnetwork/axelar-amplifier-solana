//! Type definitions for the Merkle tree primitives used in this crate.

use arrayref::mut_array_refs;
pub use rs_merkle::{MerkleProof, MerkleTree};
use udigest::encoding::EncodeValue;

#[derive(Copy, Clone)]
pub struct Hasher;

impl rs_merkle::Hasher for Hasher {
    type Hash = [u8; 32];

    fn hash(data: &[u8]) -> Self::Hash {
        solana_keccak_hasher::hash(data).0
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
        solana_keccak_hasher::hash(&concatenated).0
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
        solana_keccak_hasher::hash(&buffer.0).0
    }
}
