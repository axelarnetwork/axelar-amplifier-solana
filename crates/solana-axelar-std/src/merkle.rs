use rs_merkle::MerkleTree;
use udigest::encoding::EncodeValue;

use crate::hasher::leaf::VecBuf;

pub(crate) fn merkle_tree<'a, T: rs_merkle::Hasher, K: udigest::Digestable + 'a>(
    leaves: impl Iterator<Item = &'a K>,
) -> MerkleTree<T> {
    let leaves = leaves
        .map(|item| {
            let mut buffer = VecBuf(vec![]);
            item.unambiguously_encode(EncodeValue::new(&mut buffer));
            T::hash(&buffer.0)
        })
        .collect::<Vec<_>>();
    MerkleTree::<T>::from_leaves(&leaves)
}
