use std::collections::BTreeMap;
use udigest::Digestable;

#[cfg(feature = "anchor")]
use anchor_lang::prelude::borsh;

use crate::{hasher::LeafHash, EncodingError, PublicKey, Signature};

/// Represents a set of verifiers, each with an associated weight, and a quorum
/// value.
///
/// The `VerifierSet` struct encapsulates a collection of verifiers identified
/// by their public keys, each assigned a specific weight. Additionally, it
/// includes a quorum value that may be used to determine consensus requirements
/// within the set.
#[derive(Debug, Eq, PartialEq, Clone, borsh::BorshDeserialize, borsh::BorshSerialize)]
pub struct VerifierSet {
    /// A nonce value that can be used to track changes or updates to the
    /// verifier set.
    pub nonce: u64,

    /// A map of public keys to their corresponding weights. Each entry
    /// represents a verifier and the weight assigned to their contribution
    /// or authority.
    pub signers: BTreeMap<PublicKey, u128>,

    /// The quorum value required for consensus or decision-making within the
    /// verifier set. This value typically represents the minimum total
    /// weight needed to approve an action.
    pub quorum: u128,
}

pub type VerifierSetHash = [u8; 32];

#[derive(Clone, Copy, PartialEq, Eq, Digestable, Debug)]
#[cfg_attr(
    not(feature = "anchor"),
    derive(borsh::BorshDeserialize, borsh::BorshSerialize)
)]
#[cfg_attr(
    feature = "anchor",
    derive(anchor_lang::AnchorSerialize, anchor_lang::AnchorDeserialize)
)]
pub struct VerifierSetLeaf {
    /// The nonce value from the associated `VerifierSet`.
    pub nonce: u64,

    /// The quorum value from the associated `VerifierSet`.
    pub quorum: u128,

    /// The public key of the verifier.
    pub signer_pubkey: PublicKey,

    /// The weight assigned to the verifier, representing their voting power or
    /// authority.
    pub signer_weight: u128,

    /// The position of this leaf within the Merkle tree.
    pub position: u16,

    /// The total number of leaves in the Merkle tree, representing the size of
    /// the verifier set.
    pub set_size: u16,

    /// A domain separator used to ensure the uniqueness of hashes across
    /// different contexts.
    pub domain_separator: [u8; 32],
}

impl LeafHash for VerifierSetLeaf {}

/// Contains information about a single verifier within the signing verifier
/// set.
///
/// This struct holds the verifier's signature, their corresponding leaf in the
/// verifier set Merkle tree, and the Merkle proof needed to verify their
/// inclusion in the set.
#[derive(Debug, Eq, PartialEq, Clone)]
#[cfg_attr(
    not(feature = "anchor"),
    derive(borsh::BorshDeserialize, borsh::BorshSerialize)
)]
#[cfg_attr(
    feature = "anchor",
    derive(anchor_lang::AnchorSerialize, anchor_lang::AnchorDeserialize)
)]
pub struct SigningVerifierSetInfo {
    /// The signature provided by the verifier.
    pub signature: Signature,

    /// The leaf node representing the verifier in the Merkle tree.
    pub leaf: VerifierSetLeaf,

    /// The Merkle proof demonstrating the verifier's inclusion in the signing
    /// verifier set.
    pub merkle_proof: Vec<u8>,
}

/// Generates the Merkle root hash for a given verifier set.
///
/// The `verifier_set_hash` function constructs a Merkle tree from the leaves
/// generated from the provided `VerifierSet` and returns the Merkle root. This
/// root can be used to verify the integrity and membership of verifiers within
/// the set.
///
/// # Errors
/// - if the verifier set has no entries in it
pub fn verifier_set_hash<T: rs_merkle::Hasher>(
    verifier_set: &VerifierSet,
    domain_separator: &[u8; 32],
) -> Result<T::Hash, EncodingError> {
    let leaves = merkle_tree_leaves(verifier_set, domain_separator)?.collect::<Vec<_>>();
    let tree = crate::merkle::merkle_tree::<T, VerifierSetLeaf>(leaves.iter());

    tree.root()
        .ok_or(EncodingError::CannotMerklizeEmptyVerifierSet)
}

pub(crate) fn merkle_tree_leaves<'a>(
    vs: &'a VerifierSet,
    domain_separator: &'a [u8; 32],
) -> Result<impl Iterator<Item = VerifierSetLeaf> + 'a, EncodingError> {
    let set_size = vs
        .signers
        .len()
        .try_into()
        .map_err(|_err| EncodingError::SetSizeTooLarge)?;
    let iterator =
        vs.signers
            .iter()
            .enumerate()
            .map(
                move |(position, (signer_pubkey, signer_weight))| VerifierSetLeaf {
                    nonce: vs.nonce,
                    quorum: vs.quorum,
                    domain_separator: *domain_separator,
                    signer_pubkey: *signer_pubkey,
                    signer_weight: *signer_weight,
                    position: position
                        .try_into()
                        .expect("position and set size are guaranteed to be equal"),
                    set_size,
                },
            );
    Ok(iterator)
}
