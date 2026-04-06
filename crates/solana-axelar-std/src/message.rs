use udigest::Digestable;

use crate::{hasher::LeafHash, EncodingError};

/// Identifies a specific blockchain and its unique identifier within that
/// chain.
#[derive(
    Clone, PartialEq, Eq, Debug, Digestable, borsh::BorshSerialize, borsh::BorshDeserialize,
)]
pub struct CrossChainId {
    /// The name or identifier of the source blockchain.
    pub chain: String,

    /// A unique identifier within the specified blockchain.
    pub id: String,
}

#[cfg(feature = "idl-build")]
impl anchor_lang::IdlBuild for CrossChainId {}

/// Represents a message intended for cross-chain communication.
#[derive(
    Clone, PartialEq, Eq, Debug, Digestable, borsh::BorshSerialize, borsh::BorshDeserialize,
)]
pub struct Message {
    /// The cross-chain identifier of the message
    pub cc_id: CrossChainId,

    /// The source address from which the message originates.
    pub source_address: String,

    /// The destination blockchain where the message is intended to be sent.
    pub destination_chain: String,

    /// The destination address on the target blockchain.
    pub destination_address: String,

    /// A 32-byte hash of the message payload, ensuring data integrity.
    pub payload_hash: [u8; 32],
}

impl LeafHash for Message {}

#[cfg(feature = "idl-build")]
impl anchor_lang::IdlBuild for Message {}

impl Message {
    pub fn command_id(&self) -> [u8; 32] {
        let cc_id = &self.cc_id;
        solana_keccak_hasher::hashv(&[cc_id.chain.as_bytes(), b"-", cc_id.id.as_bytes()]).to_bytes()
    }
}

/// Represents a collection of `Message` instances.
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Messages(pub Vec<Message>);

/// Represents a leaf node in a Merkle tree for a `Message`.
///
/// The `MessageLeaf` struct includes the message itself along with metadata
/// required for Merkle tree operations, such as its position within the tree,
/// the total size of the set, and a domain separator.
#[derive(
    Clone, PartialEq, Eq, Debug, Digestable, borsh::BorshSerialize, borsh::BorshDeserialize,
)]
pub struct MessageLeaf {
    /// The message contained within this leaf node.
    pub message: Message,

    /// The position of this leaf within the Merkle tree.
    pub position: u16,

    /// The total number of leaves in the Merkle tree.
    pub set_size: u16,

    /// A domain separator used to ensure the uniqueness of hashes across
    /// different contexts.
    pub domain_separator: [u8; 32],
}

impl LeafHash for MessageLeaf {}

#[cfg(feature = "idl-build")]
impl anchor_lang::IdlBuild for MessageLeaf {}

/// Generates an iterator of `MessageLeaf` instances from a collection of
/// messages.
pub(crate) fn merkle_tree_leaves(
    messages: Messages,
    domain_separator: [u8; 32],
) -> Result<impl Iterator<Item = MessageLeaf>, EncodingError> {
    let set_size = messages
        .0
        .len()
        .try_into()
        .map_err(|_err| EncodingError::SetSizeTooLarge)?;
    let iterator = messages
        .0
        .into_iter()
        .enumerate()
        .map(move |(position, message)| MessageLeaf {
            domain_separator,
            position: position
                .try_into()
                .expect("position guaranteed to equal set size"),
            set_size,
            message,
        });
    Ok(iterator)
}

/// Represents a single message within the payload, along with its Merkle proof.
///
/// Each `MerklizedMessage` includes the message content encapsulated in a
/// `MessageLeaf` and a proof that verifies the message's inclusion in the
/// Merkle tree.
#[derive(Debug, Eq, PartialEq, Clone, borsh::BorshSerialize, borsh::BorshDeserialize)]
pub struct MerklizedMessage {
    /// The leaf node representing the message in the Merkle tree.
    pub leaf: MessageLeaf,

    /// The Merkle proof demonstrating the message's inclusion in the payload's
    /// Merkle tree.
    pub proof: Vec<u8>,
}

#[cfg(feature = "idl-build")]
impl anchor_lang::IdlBuild for MerklizedMessage {}
