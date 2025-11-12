use std::collections::BTreeMap;

use borsh::{BorshDeserialize, BorshSerialize};
use rs_merkle::MerkleTree;
use udigest::encoding::EncodeValue;

use crate::{
    hasher::leaf::VecBuf,
    message::{MerklizedMessage, MessageLeaf, Messages},
    verifier_set::{self, verifier_set_hash, SigningVerifierSetInfo},
    EncodingError, PublicKey, Signature, VerifierSet, VerifierSetLeaf,
};

/// Represents the complete set of execution data required for verification and
/// processing.
///
/// `ExecuteData` includes Merkle roots for the signing verifier set and the
/// payload, as well as detailed information about each verifier's signature and
/// the structure of the payload.
#[derive(Debug, Eq, PartialEq, Clone, BorshSerialize, BorshDeserialize)]
pub struct ExecuteData {
    /// The Merkle root of the signing verifier set.
    pub signing_verifier_set_merkle_root: [u8; 32],

    /// A list of information about each verifier in the signing set, including
    /// their signatures and Merkle proofs.
    pub signing_verifier_set_leaves: Vec<SigningVerifierSetInfo>,

    /// The Merkle root of the payload data.
    pub payload_merkle_root: [u8; 32],

    /// The payload items, which can either be new messages or a verifier set
    /// rotation, each accompanied by their respective Merkle proofs.
    pub payload_items: MerklizedPayload,
}

/// Represents the different types of payloads that can be processed within the
/// system.
#[derive(Debug, Eq, PartialEq, Clone)]
pub enum Payload {
    /// Encapsulates a collection of messages to be processed.
    Messages(Messages),

    /// Represents an updated verifier set for system consensus.
    NewVerifierSet(VerifierSet),
}

/// Represents the payload data in a Merkle tree structure.
///
/// `MerklizedPayload` can either be a rotation of the verifier set or a
/// collection of new messages, each accompanied by their respective Merkle
/// proofs.
#[derive(Debug, Eq, PartialEq, Clone, BorshSerialize, BorshDeserialize)]
pub enum MerklizedPayload {
    /// Indicates a rotation of the verifier set, providing the new Merkle root
    /// of the verifier set.
    VerifierSetRotation {
        /// The Merkle root of the new verifier set after rotation.
        new_verifier_set_merkle_root: [u8; 32],
    },

    /// Contains a list of new messages, each with its corresponding Merkle
    /// proof.
    NewMessages {
        /// A vector of `MerklizedMessage` instances, each representing a
        /// message and its proof.
        messages: Vec<MerklizedMessage>,
    },
}

/// Encodes `execute_data` components using a custom verifier set, signers, and
/// a domain separator.
///
/// # Errors
/// - IO Error when encoding the data
/// - Verifier Set has too many items in it
/// - Verifier Set has no items in it
/// - Payload messages have too many items in it
/// - Payload messages has no items in it
pub fn encode<T: rs_merkle::Hasher<Hash = [u8; 32]>>(
    signing_verifier_set: &VerifierSet,
    signers_with_signatures: &BTreeMap<PublicKey, Signature>,
    domain_separator: [u8; 32],
    payload: Payload,
) -> Result<Vec<u8>, EncodingError> {
    let leaves = verifier_set::merkle_tree_leaves(signing_verifier_set, &domain_separator)?
        .collect::<Vec<_>>();
    let signer_merkle_tree = merkle_tree::<T, VerifierSetLeaf>(leaves.iter());
    let signing_verifier_set_merkle_root = signer_merkle_tree
        .root()
        .ok_or(EncodingError::CannotMerklizeEmptyVerifierSet)?;
    let (payload_merkle_root, payload_items) =
        hash_payload_internal::<T>(payload, domain_separator)?;

    let signing_verifier_set_leaves = leaves
        .into_iter()
        .filter_map(|leaf| {
            if let Some(signature) = signers_with_signatures.get(&leaf.signer_pubkey) {
                let merkle_proof = signer_merkle_tree.proof(&[leaf.position.into()]);
                return Some(SigningVerifierSetInfo {
                    signature: *signature,
                    leaf,
                    merkle_proof: merkle_proof.to_bytes(),
                });
            }
            None
        })
        .collect::<Vec<_>>();
    let execute_data = ExecuteData {
        signing_verifier_set_merkle_root,
        signing_verifier_set_leaves,
        payload_merkle_root,
        payload_items,
    };
    let capacity = estimate_size(&execute_data);
    let mut buffer = Vec::with_capacity(capacity);
    borsh::to_writer(&mut buffer, &execute_data)?;
    Ok(buffer)
}

fn estimate_size(execute_data: &ExecuteData) -> usize {
    size_of::<ExecuteData>()
        .saturating_add({
            // estimate heap allocations
            match &execute_data.payload_items {
                MerklizedPayload::VerifierSetRotation { .. } => 0,
                MerklizedPayload::NewMessages { messages } => {
                    size_of::<MerklizedMessage>()
                        .saturating_mul(messages.len())
                        .saturating_mul({
                            // allocate for 4 hashes
                            let avg_proof_size = size_of::<[u8; 32]>().saturating_mul(4);
                            // average extra heap allocations by all the Strings in the Message
                            // struct
                            let avg_message_size = 256_usize;
                            avg_message_size.saturating_add(avg_proof_size)
                        })
                }
            }
        })
        .saturating_add(
            size_of::<SigningVerifierSetInfo>()
                .saturating_mul(execute_data.signing_verifier_set_leaves.len()),
        )
}

/// Hashes a payload, generating a unique root hash for payload validation.
///
/// # Errors
/// - When the verifier set is empty
/// - When the verifier set is too large
pub fn hash_payload<T: rs_merkle::Hasher<Hash = [u8; 32]>>(
    domain_separator: &[u8; 32],
    payload: Payload,
) -> Result<T::Hash, EncodingError> {
    let (payload_hash, _merklesied_payload) =
        hash_payload_internal::<T>(payload, *domain_separator)?;
    Ok(payload_hash)
}

/// Internal function for hashing payloads, which calculates the root and items
/// for Merklized payloads, either messages or a new verifier set.
fn hash_payload_internal<T: rs_merkle::Hasher<Hash = [u8; 32]>>(
    payload: Payload,
    domain_separator: [u8; 32],
) -> Result<(T::Hash, MerklizedPayload), EncodingError> {
    let (payload_merkle_root, payload_items) = match payload {
        Payload::Messages(messages) => {
            let leaves =
                crate::message::merkle_tree_leaves(messages, domain_separator)?.collect::<Vec<_>>();
            let messages_merkle_tree = merkle_tree::<T, MessageLeaf>(leaves.iter());
            let messages_merkle_root = messages_merkle_tree
                .root()
                .ok_or(EncodingError::CannotMerklizeEmptyMessageSet)?;
            let messages = leaves
                .into_iter()
                .map(|leaf| {
                    let proof = messages_merkle_tree.proof(&[leaf.position.into()]);
                    MerklizedMessage {
                        leaf,
                        proof: proof.to_bytes(),
                    }
                })
                .collect::<Vec<_>>();
            (
                messages_merkle_root,
                MerklizedPayload::NewMessages { messages },
            )
        }
        Payload::NewVerifierSet(verifier_set) => {
            let new_verifier_set_merkle_root =
                verifier_set_hash::<T>(&verifier_set, &domain_separator)?;
            let payload = MerklizedPayload::VerifierSetRotation {
                new_verifier_set_merkle_root,
            };
            (new_verifier_set_merkle_root, payload)
        }
    };
    Ok((payload_merkle_root, payload_items))
}

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
