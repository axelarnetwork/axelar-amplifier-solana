use borsh::{BorshDeserialize, BorshSerialize};
use rs_merkle::MerkleTree;
use std::collections::BTreeMap;
use udigest::encoding::EncodeValue;

use crate::{
    hasher::{Hasher, VecBuf},
    message::{MerklizedMessage, MessageLeaf, Messages},
    verifier_set::{self, verifier_set_hash, SigningVerifierSetInfo},
    EncodingError, PublicKey, Signature, VerifierSet, VerifierSetLeaf,
};

/// Prefix used for off-chain Solana message signing
pub const SOLANA_OFFCHAIN_PREFIX: &[u8] = b"\xffsolana offchain";

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

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(
    not(feature = "anchor"),
    derive(borsh::BorshDeserialize, borsh::BorshSerialize)
)]
#[cfg_attr(
    feature = "anchor",
    derive(anchor_lang::AnchorSerialize, anchor_lang::AnchorDeserialize)
)]
pub enum PayloadType {
    ApproveMessages = 0,
    RotateSigners = 1,
}

impl From<PayloadType> for u8 {
    fn from(payload_type: PayloadType) -> Self {
        match payload_type {
            PayloadType::ApproveMessages => 0,
            PayloadType::RotateSigners => 1,
        }
    }
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
pub fn encode(
    signing_verifier_set: &VerifierSet,
    signers_with_signatures: &BTreeMap<PublicKey, Signature>,
    domain_separator: [u8; 32],
    payload: Payload,
) -> Result<Vec<u8>, EncodingError> {
    let payload_type = match payload {
        Payload::Messages(_) => PayloadType::ApproveMessages,
        Payload::NewVerifierSet(_) => PayloadType::RotateSigners,
    };

    // Verifier Set Merkle Tree
    let leaves = verifier_set::merkle_tree_leaves(signing_verifier_set, &domain_separator)?
        .collect::<Vec<_>>();
    let signer_merkle_tree = merkle_tree::<Hasher, VerifierSetLeaf>(leaves.iter());
    let signing_verifier_set_merkle_root = signer_merkle_tree
        .root()
        .ok_or(EncodingError::CannotMerklizeEmptyVerifierSet)?;

    let signing_verifier_set_leaves = leaves
        .into_iter()
        .filter_map(|leaf| {
            if let Some(signature) = signers_with_signatures.get(&leaf.signer_pubkey) {
                let merkle_proof = signer_merkle_tree.proof(&[leaf.position.into()]);
                return Some(SigningVerifierSetInfo {
                    signature: *signature,
                    leaf,
                    merkle_proof: merkle_proof.to_bytes(),
                    payload_type,
                });
            }
            None
        })
        .collect::<Vec<_>>();

    // Payload Merkle Tree
    let (payload_merkle_root, payload_items) =
        hash_payload_internal::<Hasher>(payload, domain_separator)?;

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
            execute_data
                .signing_verifier_set_leaves
                .iter()
                .map(|info| {
                    size_of::<SigningVerifierSetInfo>().saturating_add(info.merkle_proof.len())
                })
                .sum::<usize>(),
        )
}

#[allow(clippy::indexing_slicing)]
pub fn prefixed_message_hash_payload_type(
    payload_type: PayloadType,
    message: &[u8; 32],
) -> [u8; 32] {
    // Hash the prefixed message to get a 32-byte digest
    solana_keccak_hasher::hashv(&[
        // 1. Add Solana offchain prefix to the message
        SOLANA_OFFCHAIN_PREFIX,
        // 2. Add payload type prefix to the message to indicate the intent of the signer
        // this prevents rotating signers with a payload_merkle_root intended for approving
        // messages and vice versa
        &[payload_type.into()],
        // 3. Add the original message
        message,
    ])
    .to_bytes()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CrossChainId, Message, Messages};
    use crate::{ECDSA_RECOVERABLE_SIGNATURE_LEN, SECP256K1_COMPRESSED_PUBKEY_LEN};

    fn create_test_verifier_set(num_verifiers: usize) -> VerifierSet {
        let signers = (0..num_verifiers)
            .map(|i| {
                (
                    PublicKey([u8::try_from(i).unwrap(); SECP256K1_COMPRESSED_PUBKEY_LEN]),
                    1u128,
                )
            })
            .collect();

        VerifierSet {
            nonce: 0,
            signers,
            quorum: num_verifiers as u128,
        }
    }

    fn create_test_signatures(verifier_set: &VerifierSet) -> BTreeMap<PublicKey, Signature> {
        verifier_set
            .signers
            .keys()
            .map(|pubkey| (*pubkey, Signature([0u8; ECDSA_RECOVERABLE_SIGNATURE_LEN])))
            .collect()
    }

    #[test]
    fn estimate_size_accounts_for_merkle_proofs() {
        let domain_separator = [1u8; 32];
        let verifier_set = create_test_verifier_set(10);
        let signatures = create_test_signatures(&verifier_set);
        let payload = Payload::Messages(Messages(vec![Message {
            cc_id: CrossChainId {
                chain: "test-chain".to_owned(),
                id: "1".to_owned(),
            },
            source_address: "source".to_owned(),
            destination_address: "dest".to_owned(),
            destination_chain: "chain".to_owned(),
            payload_hash: [2u8; 32],
        }]));

        let encoded = encode(&verifier_set, &signatures, domain_separator, payload)
            .expect("encoding should succeed");

        let execute_data: ExecuteData = borsh::BorshDeserialize::try_from_slice(&encoded)
            .expect("deserialization should succeed");

        let estimated_size = estimate_size(&execute_data);

        assert!(encoded.len() <= estimated_size,);

        let total_merkle_proof_bytes: usize = execute_data
            .signing_verifier_set_leaves
            .iter()
            .map(|info| info.merkle_proof.len())
            .sum();

        assert!(total_merkle_proof_bytes > 0);

        let size_without_proofs = size_of::<ExecuteData>()
            + execute_data
                .signing_verifier_set_leaves
                .iter()
                .map(|_| size_of::<SigningVerifierSetInfo>())
                .sum::<usize>();

        assert!(estimated_size >= size_without_proofs + total_merkle_proof_bytes);
    }

    #[test]
    fn estimate_size_prevents_reallocations() {
        let domain_separator = [1u8; 32];
        let verifier_set = create_test_verifier_set(5);
        let signatures = create_test_signatures(&verifier_set);
        let payload = Payload::Messages(Messages(
            (0..3)
                .map(|i| Message {
                    cc_id: CrossChainId {
                        chain: format!("chain-{i}"),
                        id: i.to_string(),
                    },
                    source_address: format!("source-{i}"),
                    destination_address: format!("dest-{i}"),
                    destination_chain: format!("chain-{i}"),
                    payload_hash: [i; 32],
                })
                .collect(),
        ));

        let encoded = encode(&verifier_set, &signatures, domain_separator, payload)
            .expect("encoding should succeed");

        let execute_data: ExecuteData = borsh::BorshDeserialize::try_from_slice(&encoded)
            .expect("deserialization should succeed");

        let estimated_size = estimate_size(&execute_data);

        assert!(encoded.len() <= estimated_size);
    }

    #[test]
    fn estimate_size_scales_with_merkle_proof_size() {
        let domain_separator = [1u8; 32];

        // Test with different numbers of verifiers (which affects merkle proof size)
        for num_verifiers in [2, 4, 8, 16, 32] {
            let verifier_set = create_test_verifier_set(num_verifiers);
            let signatures = create_test_signatures(&verifier_set);
            let payload = Payload::Messages(Messages(vec![Message {
                cc_id: CrossChainId {
                    chain: "test".to_owned(),
                    id: "1".to_owned(),
                },
                source_address: "src".to_owned(),
                destination_address: "dst".to_owned(),
                destination_chain: "chain".to_owned(),
                payload_hash: [0u8; 32],
            }]));

            let encoded = encode(&verifier_set, &signatures, domain_separator, payload)
                .expect("encoding should succeed");

            let execute_data: ExecuteData = borsh::BorshDeserialize::try_from_slice(&encoded)
                .expect("deserialization should succeed");

            let estimated_size = estimate_size(&execute_data);

            assert!(encoded.len() <= estimated_size);
        }
    }
}
