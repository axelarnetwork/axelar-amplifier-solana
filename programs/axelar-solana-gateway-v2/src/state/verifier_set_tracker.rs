use crate::{VerifierSetHash, U256};
use anchor_lang::prelude::*;

/// Ever-incrementing counter for keeping track of the sequence of signer sets
pub type Epoch = U256;

#[account(zero_copy)]
#[derive(Debug, PartialEq, Eq)]
#[allow(clippy::pub_underscore_fields)]
pub struct VerifierSetTracker {
    /// The canonical bump for this account.
    pub bump: u8,
    /// Padding for the bump
    pub _padding: [u8; 7],
    /// The epoch associated with this verifier set
    pub epoch: Epoch,
    /// The verifier set hash
    pub verifier_set_hash: VerifierSetHash,
}
