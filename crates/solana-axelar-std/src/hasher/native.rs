#![cfg(any(feature = "sha3", test))]

use crate::hasher::{concat_and_hash, HashvSupport};

/// A Merkle Tree hasher that uses the native `sha3` crate's `Keccak256` hashing
/// algorithm.
///
/// The `NativeHasher` is suitable for environments outside of Solana, providing
/// a reliable and efficient hashing mechanism for Merkle tree operations.
#[derive(Copy, Clone)]
pub struct NativeHasher;

impl HashvSupport for NativeHasher {
    fn hashv(vals: &[&[u8]]) -> [u8; 32] {
        use sha3::digest::Digest;
        let mut hasher = sha3::Keccak256::default();
        for val in vals {
            hasher.update(val);
        }
        let res = hasher.finalize();
        res.into()
    }
}

impl rs_merkle::Hasher for NativeHasher {
    type Hash = [u8; 32];

    fn hash(data: &[u8]) -> Self::Hash {
        use sha3::digest::Digest;
        let mut hasher = sha3::Keccak256::default();
        hasher.update(data);
        hasher.finalize().into()
    }

    fn concat_and_hash(left: &Self::Hash, right: Option<&Self::Hash>) -> Self::Hash {
        concat_and_hash(left, right, Self::hash)
    }
}
