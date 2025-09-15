use crate::{u256::U256, GatewayError};
use anchor_lang::prelude::{
    borsh::{BorshDeserialize, BorshSerialize},
    *,
};

/// Timestamp alias for when the last signer rotation happened
pub type Timestamp = u64;
/// Seconds that need to pass between signer rotations
pub type RotationDelaySecs = u64;
/// Ever-incrementing idx for the signer set
pub type VerifierSetEpoch = U256;

#[account]
#[derive(Debug, PartialEq, Eq)]
pub struct GatewayConfig {
    pub current_epoch: VerifierSetEpoch,
    pub previous_verifier_set_retention: VerifierSetEpoch,
    pub minimum_rotation_delay: RotationDelaySecs,
    pub last_rotation_timestamp: Timestamp,
    pub operator: Pubkey,
    pub domain_separator: [u8; 32],
    pub bump: u8,
}

impl GatewayConfig {
    pub fn assert_valid_epoch(&self, epoch: U256) -> Result<()> {
        let current_epoch = self.current_epoch;
        let elapsed = current_epoch
            .checked_sub(epoch)
            .ok_or(GatewayError::EpochCalculationOverflow)?;

        if elapsed >= self.previous_verifier_set_retention {
            return err!(GatewayError::VerifierSetTooOld);
        }
        Ok(())
    }
}

pub type VerifierSetHash = [u8; 32];

/// Represents an initial verifier set with its hash and PDA
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct InitialVerifierSet {
    /// The hash of the verifier set
    pub hash: VerifierSetHash,
    /// The PDA for the verifier set tracker
    pub pda: Pubkey,
}

#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct InitializeConfig {
    _padding: u8,
    /// The domain separator, used as an input for hashing payloads.
    pub domain_separator: [u8; 32],
    /// initial verifier set
    pub initial_verifier_set: InitialVerifierSet,
    /// the minimum delay required between rotations
    pub minimum_rotation_delay: RotationDelaySecs,
    /// The gateway operator.
    pub operator: Pubkey,
    /// how many n epochs do we consider valid
    pub previous_verifier_retention: VerifierSetEpoch,
}

impl InitializeConfig {
    pub fn new(
        domain_separator: [u8; 32],
        initial_verifier_set: InitialVerifierSet,
        minimum_rotation_delay: RotationDelaySecs,
        operator: Pubkey,
        previous_verifier_retention: VerifierSetEpoch,
    ) -> Self {
        Self {
            _padding: 0,
            domain_separator,
            initial_verifier_set,
            minimum_rotation_delay,
            operator,
            previous_verifier_retention,
        }
    }
}

/// Ever-incrementing counter for keeping track of the sequence of signer sets
pub type Epoch = U256;

#[account]
#[derive(Debug, PartialEq, Eq)]
pub struct VerifierSetTracker {
    /// The canonical bump for this account.
    pub bump: u8,
    /// The epoch associated with this verifier set
    pub epoch: Epoch,
    /// The verifier set hash
    pub verifier_set_hash: VerifierSetHash,
}
