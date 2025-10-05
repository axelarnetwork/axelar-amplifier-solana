use crate::U128;
use crate::{GatewayError, PublicKey, VecBuf, VerifierSetHash};
use anchor_lang::prelude::*;
use anchor_lang::solana_program;
use axelar_solana_encoding::{hasher::SolanaSyscallHasher, rs_merkle};
use bitvec::prelude::*;
use bytemuck::{Pod, Zeroable};
use udigest::{encoding::EncodeValue, Digestable};

/// This PDA tracks that all the signatures for a given payload get verified
#[account(zero_copy)]
#[derive(Debug, PartialEq, Eq, Default)]
#[allow(clippy::pub_underscore_fields)]
pub struct SignatureVerificationSessionData {
    /// Signature verification session
    pub signature_verification: SignatureVerification,
    /// Seed bump for this account's PDA
    pub bump: u8,
    /// Padding for memory alignment.
    pub _pad: [u8; 15],
}

/// Controls the signature verification session for a given payload.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Pod, Zeroable)]
pub struct SignatureVerification {
    /// Accumulated signer threshold required to validate the payload.
    ///
    /// Is incremented on each successful verification.
    ///
    /// Set to [`U128::MAX`] once the accumulated threshold is greater than or
    /// equal the current verifier set threshold.
    pub accumulated_threshold: U128,

    /// A bit field used to track which signatures have been verified.
    ///
    /// Initially, all bits are set to zero. When a signature is verified, its
    /// corresponding bit is flipped to one. This prevents the same signature
    /// from being verified more than once, avoiding deliberate attempts to
    /// decrement the remaining threshold.
    ///
    /// Currently supports 256 slots. If the signer set maximum size needs to be
    /// increased in the future, this value must change to make roof for
    /// them.
    pub signature_slots: [u8; 32],

    /// Upon the first successful signature validation, we set the hash of the
    /// signing verifier set.
    /// This data is later used when rotating signers to figure out which
    /// verifier set was the one that actually performed the validation.
    pub signing_verifier_set_hash: VerifierSetHash,
}

impl SignatureVerification {
    pub fn is_valid(&self) -> bool {
        self.accumulated_threshold == U128::MAX
    }
}

impl SignatureVerificationSessionData {
    pub fn new(signature_verification: SignatureVerification, bump: u8) -> Self {
        SignatureVerificationSessionData {
            signature_verification,
            bump,
            ..Default::default()
        }
    }

    pub fn is_valid(&self) -> bool {
        self.signature_verification.is_valid()
    }

    pub fn process_signature(
        &mut self,
        payload_merkle_root: [u8; 32],
        verifier_set_merkle_root: &[u8; 32],
        verifier_info: SigningVerifierSetInfo,
    ) -> Result<()> {
        // Check: Slot is already verified
        self.check_slot_is_done(&verifier_info.leaf)?;

        // Check: Digital signature
        let pubkey_bytes = match verifier_info.leaf.signer_pubkey {
            PublicKey::Secp256k1(key) => key,
            PublicKey::Ed25519(_) => return err!(GatewayError::UnsupportedSignatureScheme),
        };

        if !Self::verify_ecdsa_signature(
            &pubkey_bytes,
            &verifier_info.signature,
            &payload_merkle_root,
        ) {
            return err!(GatewayError::SignatureVerificationFailed);
        }

        // Check: Merkle proof
        let merkle_proof =
            rs_merkle::MerkleProof::<SolanaSyscallHasher>::from_bytes(&verifier_info.merkle_proof)
                .map_err(|_err| GatewayError::InvalidMerkleProof)?;

        let leaf_hash = verifier_info.leaf.hash();

        if !merkle_proof.verify(
            *verifier_set_merkle_root,
            &[verifier_info.leaf.position.into()],
            &[leaf_hash],
            verifier_info.leaf.set_size.into(),
        ) {
            return err!(GatewayError::InvalidMerkleProof);
        }

        // Update state
        self.accumulate_threshold(&verifier_info.leaf)?;
        self.mark_slot_done(&verifier_info.leaf)?;
        self.verify_or_initialize_verifier_set(verifier_set_merkle_root)?;

        Ok(())
    }

    pub fn verify_ecdsa_signature(
        pubkey: &axelar_solana_encoding::types::pubkey::Secp256k1Pubkey,
        signature: &axelar_solana_encoding::types::pubkey::EcdsaRecoverableSignature,
        message: &[u8; 32],
    ) -> bool {
        // The recovery bit in the signature's bytes is placed at the end, as per the
        // 'multisig-prover' contract by Axelar. Unwrap: we know the 'signature'
        // slice exact size, and it isn't empty.
        let (signature, recovery_id) = match signature {
            [first_64 @ .., recovery_id] => (first_64, recovery_id),
        };

        // Transform from Ethereum recovery_id (27, 28) to a range accepted by
        // secp256k1_recover (0, 1, 2, 3)
        // Only values 27 and 28 are valid Ethereum recovery IDs
        let recovery_id = if *recovery_id == 27 || *recovery_id == 28 {
            recovery_id.saturating_sub(27)
        } else {
            solana_program::msg!("Invalid recovery ID: {} (must be 27 or 28)", recovery_id);
            return false;
        };

        // This is results in a Solana syscall.
        let secp256k1_recover =
            solana_program::secp256k1_recover::secp256k1_recover(message, recovery_id, signature);
        let Ok(recovered_uncompressed_pubkey) = secp256k1_recover else {
            solana_program::msg!("Failed to recover ECDSA signature");
            return false;
        };

        // Unwrap: provided pubkey is guaranteed to be secp256k1 key
        let pubkey = libsecp256k1::PublicKey::parse_compressed(pubkey)
            .unwrap()
            .serialize();

        // we drop the const prefix byte that indicates that this is an uncompressed
        // pubkey
        let full_pubkey = match pubkey {
            [_tag, pubkey @ ..] => pubkey,
        };
        recovered_uncompressed_pubkey.to_bytes() == full_pubkey
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
            .saturating_add(U128::new(signature_node.signer_weight));

        // Check threshold
        if self.signature_verification.accumulated_threshold.get() >= signature_node.quorum {
            self.signature_verification.accumulated_threshold = U128::MAX;
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
#[allow(clippy::pub_underscore_fields)]
pub struct SigningVerifierSetInfo {
    pub _padding: u8,
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

#[cfg(test)]
mod tests {
    use core::mem::size_of;

    use super::*;

    #[test]
    fn test_initialization() {
        let buffer = [0_u8; size_of::<SignatureVerificationSessionData>()];
        let from_pod: &SignatureVerificationSessionData = bytemuck::cast_ref(&buffer);
        let default = &SignatureVerificationSessionData::default();
        assert_eq!(from_pod, default);
        assert_eq!(
            from_pod.signature_verification.accumulated_threshold.get(),
            0
        );
        assert_eq!(from_pod.signature_verification.signature_slots, [0_u8; 32]);
        assert!(!from_pod.signature_verification.is_valid());
    }

    #[test]
    fn test_serialization() {
        let mut buffer: [u8; size_of::<SignatureVerificationSessionData>()] =
            [42; size_of::<SignatureVerificationSessionData>()];

        let original_state;

        let updated_state = {
            let deserialized: &mut SignatureVerificationSessionData =
                bytemuck::cast_mut(&mut buffer);
            original_state = *deserialized;
            let new_threshold = deserialized
                .signature_verification
                .accumulated_threshold
                .saturating_add(U128::new(1));
            deserialized.signature_verification.accumulated_threshold = new_threshold;
            *deserialized
        };
        assert_ne!(updated_state, original_state); // confidence check

        let deserialized: &SignatureVerificationSessionData = bytemuck::cast_ref(&buffer);
        assert_eq!(&updated_state, deserialized);
    }

    #[test]
    fn test_v1_compat() {
        use axelar_solana_gateway::state::{
            signature_verification::SignatureVerification as SignatureVerificationV1,
            signature_verification_pda::SignatureVerificationSessionData as V1,
        };
        assert_eq!(
            std::mem::size_of::<SignatureVerificationSessionData>(),
            std::mem::size_of::<V1>()
        );

        // Make v2
        let signature_verification = SignatureVerification {
            accumulated_threshold: U128::new(42),
            signature_slots: [1; 32],
            signing_verifier_set_hash: [2; 32],
        };
        let bump = 255u8;
        let v2_state = SignatureVerificationSessionData::new(signature_verification, bump);

        // Make v1
        let mut v1_state = V1::default();
        v1_state.signature_verification = SignatureVerificationV1 {
            accumulated_threshold: 42_u128,
            signature_slots: [1; 32],
            signing_verifier_set_hash: [2; 32],
        };
        v1_state.bump = bump;

        // Compare byte representations
        assert_eq!(bytemuck::bytes_of(&v1_state), bytemuck::bytes_of(&v2_state));
    }

    #[test]
    fn test_alignment_compatibility() {
        // Critical: alignment must be ≤ 8 to work with Anchor's 8-byte discriminator
        // The struct data starts at byte offset 8, which is 8-byte aligned but NOT 16-byte aligned
        let alignment = std::mem::align_of::<SignatureVerificationSessionData>();
        assert!(
               alignment <= 8,
               "Struct alignment ({alignment}) must be <= 8 bytes for zero-copy compatibility with Anchor discriminator",
           );
    }
}
