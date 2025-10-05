use anchor_lang::prelude::*;
use bytemuck::{Pod, Zeroable};

/// Custom u128 type with 8-byte alignment instead of the default 16-byte alignment.
///
/// This type is required for zero-copy accounts in Anchor. The standard `u128` type
/// has 16-byte alignment, which creates a misalignment issue with Anchor's 8-byte
/// discriminator:
/// - Account discriminator occupies bytes 0-7 (8 bytes)
/// - Account data starts at byte 8
/// - With 16-byte alignment, the data at byte 8 is not properly aligned for `u128`
/// - This causes `bytemuck::from_bytes` to fail during deserialization
///
/// By using `[u8; 16]` as the underlying representation, we achieve 8-byte alignment
/// while maintaining the same byte layout as `u128` (little-endian). This makes the
/// type compatible with both Anchor's zero-copy deserialization and existing account
/// data from the previous program version that used `u128`.
///
/// The byte representation is identical to `u128`, ensuring backwards compatibility.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Pod, Zeroable)]
#[repr(C)]
pub struct U128([u8; 16]);

impl U128 {
    pub const ZERO: Self = Self([0u8; 16]);
    pub const MAX: Self = Self([0xFF; 16]);

    #[allow(clippy::little_endian_bytes)]
    pub const fn new(value: u128) -> Self {
        Self(value.to_le_bytes())
    }

    #[allow(clippy::little_endian_bytes)]
    pub const fn get(self) -> u128 {
        u128::from_le_bytes(self.0)
    }

    #[must_use]
    pub fn checked_add(self, other: Self) -> Option<Self> {
        self.get().checked_add(other.get()).map(Self::new)
    }

    #[must_use]
    pub fn saturating_add(self, other: Self) -> Self {
        Self::new(self.get().saturating_add(other.get()))
    }

    #[must_use]
    pub fn saturating_add_u128(self, other: u128) -> Self {
        Self::new(self.get().saturating_add(other))
    }

    #[must_use]
    pub fn checked_sub(self, other: Self) -> Option<Self> {
        self.get().checked_sub(other.get()).map(Self::new)
    }

    #[must_use]
    pub fn saturating_sub(self, other: Self) -> Self {
        Self::new(self.get().saturating_sub(other.get()))
    }
}

impl From<u128> for U128 {
    fn from(value: u128) -> Self {
        Self::new(value)
    }
}

impl From<U128> for u128 {
    fn from(value: U128) -> Self {
        value.get()
    }
}

impl From<u64> for U128 {
    fn from(value: u64) -> Self {
        Self::new(value as u128)
    }
}

// Implement AnchorSerialize/Deserialize to serialize as u128 in IDL
impl AnchorSerialize for U128 {
    fn serialize<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        writer.write_all(&self.0)
    }
}

impl AnchorDeserialize for U128 {
    fn deserialize_reader<R: std::io::Read>(reader: &mut R) -> std::io::Result<Self> {
        let mut bytes = [0u8; 16];
        reader.read_exact(&mut bytes)?;
        Ok(Self(bytes))
    }
}

// Display implementation
impl std::fmt::Display for U128 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.get())
    }
}
