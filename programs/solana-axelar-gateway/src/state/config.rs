use crate::GatewayError;
use anchor_lang::prelude::*;
use solana_axelar_std::U256;

/// Timestamp alias for when the last signer rotation happened
pub type Timestamp = u64;
/// Seconds that need to pass between signer rotations
pub type RotationDelaySecs = u64;
/// Ever-incrementing idx for the signer set
pub type VerifierSetEpoch = U256;

#[account(zero_copy)]
#[derive(Debug, PartialEq, Eq)]
#[allow(clippy::pub_underscore_fields)]
pub struct GatewayConfig {
    /// current epoch points to the latest signer set hash
    pub current_epoch: VerifierSetEpoch,
    /// how many n epochs do we consider valid
    pub previous_verifier_set_retention: VerifierSetEpoch,
    /// the minimum delay required between rotations
    pub minimum_rotation_delay: RotationDelaySecs,
    /// timestamp tracking of when the previous rotation happened
    pub last_rotation_timestamp: Timestamp,
    /// The gateway operator.
    pub operator: Pubkey,
    /// The domain separator, used as an input for hashing payloads.
    pub domain_separator: [u8; 32],
    /// The canonical bump for this account.
    pub bump: u8,
    /// padding for bump
    pub _padding: [u8; 7],
}

impl GatewayConfig {
    pub const SEED_PREFIX: &'static [u8] = b"gateway";

    pub fn pda_seeds<'a>() -> [&'a [u8]; 1] {
        [Self::SEED_PREFIX]
    }

    pub fn try_find_pda() -> Option<(Pubkey, u8)> {
        Pubkey::try_find_program_address(&Self::pda_seeds(), &crate::ID)
    }

    pub fn find_pda() -> (Pubkey, u8) {
        Pubkey::find_program_address(&Self::pda_seeds(), &crate::ID)
    }

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

/// Represents an initial verifier set with its hash and PDA
#[derive(Debug, Clone, PartialEq, Eq, AnchorSerialize, AnchorDeserialize)]
pub struct InitialVerifierSet {
    /// The hash of the verifier set
    pub hash: [u8; 32],
    /// The PDA for the verifier set tracker
    pub pda: Pubkey,
}

#[derive(Debug, Clone, PartialEq, Eq, AnchorSerialize, AnchorDeserialize)]
pub struct InitializeConfigParams {
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
