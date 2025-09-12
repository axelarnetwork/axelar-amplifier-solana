use anchor_lang::prelude::*;

/// Custom U256 implementation that works with Anchor
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, AnchorSerialize, AnchorDeserialize,
)]
pub struct U256 {
    // Use little-endian: [least_significant, ..., most_significant]
    inner: [u64; 4],
}

impl U256 {
    pub const ZERO: U256 = U256 { inner: [0; 4] };
    pub const ONE: U256 = U256 {
        inner: [1, 0, 0, 0],
    };

    /// Create from a u64
    pub const fn from_u64(value: u64) -> Self {
        U256 {
            inner: [value, 0, 0, 0],
        }
    }

    /// Create from [u64; 4] array
    pub const fn from_inner(inner: [u64; 4]) -> Self {
        U256 { inner }
    }

    /// Get the inner [u64; 4] representation
    pub const fn to_inner(self) -> [u64; 4] {
        self.inner
    }

    /// Convert to u64 (panics if too large)
    pub fn as_u64(self) -> u64 {
        if self.inner[1] != 0 || self.inner[2] != 0 || self.inner[3] != 0 {
            panic!("U256 value too large for u64");
        }
        self.inner[0]
    }

    /// Checked addition
    pub fn checked_add(self, other: U256) -> Option<U256> {
        let mut result = [0u64; 4];
        let mut carry = 0u64;

        for i in 0..4 {
            let sum = (self.inner[i] as u128) + (other.inner[i] as u128) + (carry as u128);
            result[i] = sum as u64;
            carry = (sum >> 64) as u64;
        }

        if carry != 0 {
            None // Overflow
        } else {
            Some(U256 { inner: result })
        }
    }

    /// Checked subtraction
    pub fn checked_sub(self, other: U256) -> Option<U256> {
        if self < other {
            return None; // Would underflow
        }

        let mut result = [0u64; 4];
        let mut borrow = 0u64;

        for i in 0..4 {
            let a = self.inner[i] as u128;
            let b = (other.inner[i] as u128) + (borrow as u128);

            if a >= b {
                result[i] = (a - b) as u64;
                borrow = 0;
            } else {
                result[i] = ((a + (1u128 << 64)) - b) as u64;
                borrow = 1;
            }
        }

        Some(U256 { inner: result })
    }
}

// Implement arithmetic operators
impl std::ops::Add for U256 {
    type Output = U256;

    fn add(self, other: U256) -> U256 {
        self.checked_add(other).expect("U256 addition overflow")
    }
}

impl std::ops::Sub for U256 {
    type Output = U256;

    fn sub(self, other: U256) -> U256 {
        self.checked_sub(other).expect("U256 subtraction underflow")
    }
}

// Implement AddAssign, SubAssign, etc.
impl std::ops::AddAssign for U256 {
    fn add_assign(&mut self, other: U256) {
        *self = *self + other;
    }
}

impl std::ops::SubAssign for U256 {
    fn sub_assign(&mut self, other: U256) {
        *self = *self - other;
    }
}

// From conversions
impl From<u64> for U256 {
    fn from(value: u64) -> Self {
        U256::from_u64(value)
    }
}

impl From<[u64; 4]> for U256 {
    fn from(inner: [u64; 4]) -> Self {
        U256::from_inner(inner)
    }
}

impl Into<[u64; 4]> for U256 {
    fn into(self) -> [u64; 4] {
        self.inner
    }
}
