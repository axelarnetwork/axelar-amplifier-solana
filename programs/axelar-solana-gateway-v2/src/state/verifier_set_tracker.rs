use crate::{VerifierSetHash, U256};
use anchor_lang::prelude::*;

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
