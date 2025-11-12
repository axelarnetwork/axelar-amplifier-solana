use udigest::encoding::EncodeValue;

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
    fn hash<T: rs_merkle::Hasher>(&self) -> T::Hash {
        let mut buffer = VecBuf(vec![]);
        self.unambiguously_encode(EncodeValue::new(&mut buffer));
        T::hash(&buffer.0)
    }
}
