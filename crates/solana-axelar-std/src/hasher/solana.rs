#![cfg(any(feature = "solana", test))]

use crate::hasher::{concat_and_hash, HashvSupport};

/// A Merkle Tree hasher that utilizes Solana's `keccak` syscall to merge nodes.

#[derive(Copy, Clone)]
pub struct SolanaSyscallHasher;

impl HashvSupport for SolanaSyscallHasher {
    fn hashv(data: &[&[u8]]) -> [u8; 32] {
        solana_keccak_hasher::hashv(data).to_bytes()
    }
}

impl rs_merkle::Hasher for SolanaSyscallHasher {
    type Hash = [u8; 32];

    fn hash(data: &[u8]) -> Self::Hash {
        solana_keccak_hasher::hash(data).to_bytes()
    }

    fn concat_and_hash(left: &Self::Hash, right: Option<&Self::Hash>) -> Self::Hash {
        concat_and_hash(left, right, Self::hash)
    }
}
