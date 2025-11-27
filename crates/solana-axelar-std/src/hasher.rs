//! Type definitions for the Merkle tree primitives used in this crate.

use arrayref::mut_array_refs;
pub use rs_merkle::{MerkleProof, MerkleTree};
use sha3::{Digest, Keccak256};
use udigest::encoding::EncodeValue;

fn keccak256(data: &[u8]) -> [u8; 32] {
    Keccak256::digest(data).into()
}

#[derive(Copy, Clone)]
pub struct Hasher;

impl rs_merkle::Hasher for Hasher {
    type Hash = [u8; 32];

    fn hash(data: &[u8]) -> Self::Hash {
        keccak256(data)
    }

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

pub trait LeafHash: udigest::Digestable {
    fn hash(&self) -> [u8; 32] {
        let mut buffer = VecBuf(vec![]);
        self.unambiguously_encode(EncodeValue::new(&mut buffer));
        keccak256(&buffer.0)
    }
}
