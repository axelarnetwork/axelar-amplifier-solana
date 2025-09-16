use crate::{GatewayError, PublicKey, VecBuf, VerifierSetHash};
use anchor_lang::prelude::*;
use axelar_solana_encoding::{hasher::SolanaSyscallHasher, rs_merkle};
use axelar_solana_gateway::state::signature_verification::verify_ecdsa_signature;
use bitvec::prelude::*;
use udigest::{encoding::EncodeValue, Digestable};

#[derive(Debug, Clone, PartialEq, Eq, AnchorSerialize, AnchorDeserialize)]
pub struct SignatureVerification {
    pub accumulated_threshold: u128,
    pub signature_slots: [u8; 32],
    pub signing_verifier_set_hash: VerifierSetHash,
}

impl SignatureVerification {
    pub fn is_valid(&self) -> bool {
        self.accumulated_threshold == u128::MAX
    }
}

// #[derive(Zeroable, Pod, Copy, Clone, Default, PartialEq, Eq, Debug)]
// pub struct SignatureVerificationSessionData {
//     /// Anchor compatible discriminator
//     pub discriminator: [u8; 8],
//     /// Padding to align SignatureVerification to 16-byte boundary
//     _discriminator_pad: [u8; 8],
//     /// Signature verification session
//     pub signature_verification: SignatureVerification,
//     /// Seed bump for this account's PDA
//     pub bump: u8,
//     /// Padding for memory alignment.
//     _pad: [u8; 15],
// }

#[account]
#[derive(Debug, PartialEq, Eq)]
pub struct SignatureVerificationSessionData {
    pub _discriminator_pad: [u8; 8],
    pub signature_verification: SignatureVerification,
    pub bump: u8,
    pub _pad: [u8; 15],
}

impl SignatureVerificationSessionData {
    pub fn new(signature_verification: SignatureVerification, bump: u8) -> Self {
        SignatureVerificationSessionData {
            _discriminator_pad: [0u8; 8],
            signature_verification,
            bump,
            _pad: [0u8; 15],
        }
    }

    pub fn process_signature(
        &mut self,
        payload_merkle_root: [u8; 32],
        verifier_set_merkle_root: &[u8; 32],
        verifier_info: SigningVerifierSetInfo,
    ) -> Result<()> {
        self.check_slot_is_done(&verifier_info.leaf)?;

        let pubkey_bytes = match verifier_info.leaf.signer_pubkey {
            PublicKey::Secp256k1(key) => key,
            _ => return err!(GatewayError::UnsupportedSignatureScheme),
        };

        if !verify_ecdsa_signature(
            &pubkey_bytes,
            &verifier_info.signature,
            &payload_merkle_root,
        ) {
            return err!(GatewayError::SignatureVerificationFailed);
        }

        let merkle_proof =
            rs_merkle::MerkleProof::<SolanaSyscallHasher>::from_bytes(&verifier_info.merkle_proof)
                .map_err(|_err| GatewayError::InvalidMerkleProof)?;

        // simple keccak hash for now
        let leaf_hash = verifier_info.leaf.hash();

        if !merkle_proof.verify(
            *verifier_set_merkle_root,
            &[verifier_info.leaf.position.into()],
            &[leaf_hash],
            verifier_info.leaf.set_size.into(),
        ) {
            return err!(GatewayError::InvalidMerkleProof);
        }

        self.mark_slot_done(&verifier_info.leaf)?;
        self.accumulate_threshold(&verifier_info.leaf)?;

        // Check that verifier belogs to
        self.verify_or_initialize_verifier_set(verifier_set_merkle_root)?;

        Ok(())
    }

    fn check_slot_is_done(&self, signature_node: &VerifierSetLeaf) -> Result<()> {
        let signature_slots = self
            .signature_verification
            .signature_slots
            .view_bits::<Lsb0>();
        let position: usize = signature_node.position.into();

        let Some(slot) = signature_slots.get(position) else {
            // Index is out of bounds.
            return err!(GatewayError::SlotIsOutOfBounds);
        };
        // Check if signature slot was already verified.
        if *slot {
            return err!(GatewayError::SlotAlreadyVerified);
        }
        Ok(())
    }

    fn mark_slot_done(&mut self, signature_node: &VerifierSetLeaf) -> Result<()> {
        let signature_slots = self
            .signature_verification
            .signature_slots
            .view_bits_mut::<Lsb0>();
        let position: usize = signature_node.position.into();
        let Some(slot) = signature_slots.get_mut(position) else {
            // Index is out of bounds.
            return err!(GatewayError::SlotIsOutOfBounds);
        };
        // Check if signature slot was already verified.
        if *slot {
            return err!(GatewayError::SlotAlreadyVerified);
        }
        slot.commit(true);
        Ok(())
    }

    fn accumulate_threshold(&mut self, signature_node: &VerifierSetLeaf) -> Result<()> {
        self.signature_verification.accumulated_threshold = self
            .signature_verification
            .accumulated_threshold
            .saturating_add(signature_node.signer_weight);

        // Check threshold
        if self.signature_verification.accumulated_threshold >= signature_node.quorum {
            self.signature_verification.accumulated_threshold = u128::MAX;
        }

        Ok(())
    }

    fn verify_or_initialize_verifier_set(&mut self, expected_hash: &[u8; 32]) -> Result<()> {
        if self.signature_verification.signing_verifier_set_hash == [0; 32] {
            self.signature_verification.signing_verifier_set_hash = *expected_hash;
            return Ok(());
        }

        if self.signature_verification.signing_verifier_set_hash != *expected_hash {
            return err!(GatewayError::InvalidDigitalSignature);
        }

        Ok(())
    }
}

pub type Signature = [u8; 65];

#[derive(Debug, Eq, PartialEq, Clone, AnchorSerialize, AnchorDeserialize)]
pub struct SigningVerifierSetInfo {
    _padding: u8,
    pub signature: Signature,
    pub leaf: VerifierSetLeaf,
    pub merkle_proof: Vec<u8>,
}

impl SigningVerifierSetInfo {
    pub fn new(signature: Signature, leaf: VerifierSetLeaf, merkle_proof: Vec<u8>) -> Self {
        SigningVerifierSetInfo {
            _padding: 0u8,
            signature,
            leaf,
            merkle_proof,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Digestable, Debug, AnchorSerialize, AnchorDeserialize)]
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

impl VerifierSetLeaf {
    pub fn hash(&self) -> [u8; 32] {
        let mut buffer = VecBuf(vec![]);
        self.unambiguously_encode(EncodeValue::new(&mut buffer));
        solana_program::keccak::hash(&buffer.0).to_bytes()
    }

    pub fn new(
        nonce: u64,
        quorum: u128,
        signer_pubkey: PublicKey,
        signer_weight: u128,
        position: u16,
        set_size: u16,
        domain_separator: [u8; 32],
    ) -> Self {
        Self {
            nonce,
            quorum,
            signer_pubkey,
            signer_weight,
            position,
            set_size,
            domain_separator,
        }
    }
}
