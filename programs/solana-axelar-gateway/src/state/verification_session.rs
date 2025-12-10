use crate::GatewayError;
use anchor_lang::prelude::*;
use anchor_lang::solana_program;
use bitvec::prelude::*;
use solana_axelar_std::hasher::LeafHash;
use solana_axelar_std::{
    EcdsaRecoverableSignature, MerkleProof, Secp256k1Pubkey, SigningVerifierSetInfo,
    VerifierSetLeaf, U128,
};

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

impl SignatureVerificationSessionData {
    pub const SEED_PREFIX: &'static [u8] = b"gtw-sig-verif";

    pub fn find_pda(
        payload_merkle_root: &[u8; 32],
        signing_verifier_set_hash: &[u8; 32],
    ) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[
                Self::SEED_PREFIX,
                payload_merkle_root,
                signing_verifier_set_hash,
            ],
            &crate::ID,
        )
    }

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

        // Check: Merkle proof
        let merkle_proof = MerkleProof::from_bytes(&verifier_info.merkle_proof)
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

        // Check: Digital signature
        if !Self::verify_ecdsa_signature(
            &verifier_info.leaf.signer_pubkey.0,
            &verifier_info.signature.0,
            &payload_merkle_root,
        ) {
            return err!(GatewayError::SignatureVerificationFailed);
        }

        // Update state
        self.accumulate_threshold(&verifier_info.leaf)?;
        self.mark_slot_done(&verifier_info.leaf)?;
        self.verify_verifier_set(verifier_set_merkle_root)?;

        Ok(())
    }

    /// Prefix and hash the given message
    pub fn prefixed_message_hash(message: &[u8; 32]) -> [u8; 32] {
        // Add Solana offchain prefix to the message before verification
        // This follows the Axelar convention for prefixing signed messages
        const SOLANA_OFFCHAIN_PREFIX: &[u8] = b"\xffsolana offchain";
        let mut prefixed_message = Vec::with_capacity(SOLANA_OFFCHAIN_PREFIX.len() + message.len());
        prefixed_message.extend_from_slice(SOLANA_OFFCHAIN_PREFIX);
        prefixed_message.extend_from_slice(message);

        // Hash the prefixed message to get a 32-byte digest
        solana_program::keccak::hash(&prefixed_message).to_bytes()
    }

    pub fn verify_ecdsa_signature(
        pubkey: &Secp256k1Pubkey,
        signature: &EcdsaRecoverableSignature,
        message: &[u8; 32],
    ) -> bool {
        // Hash the prefixed message to get a 32-byte digest
        let hashed_message = Self::prefixed_message_hash(message);

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
        let secp256k1_recover = solana_program::secp256k1_recover::secp256k1_recover(
            &hashed_message,
            recovery_id,
            signature,
        );
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

    #[inline]
    fn verify_verifier_set(&self, expected_hash: &[u8; 32]) -> Result<()> {
        if self.signature_verification.signing_verifier_set_hash != *expected_hash {
            return err!(GatewayError::InvalidDigitalSignature);
        }

        Ok(())
    }
}

/// Controls the signature verification session for a given payload.
#[zero_copy]
#[derive(Debug, Default, PartialEq, Eq)]
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
    /// increased in the future, this value must change to make room for
    /// them.
    pub signature_slots: [u8; 32],

    /// Upon the first successful signature validation, we set the hash of the
    /// signing verifier set.
    /// This data is later used when rotating signers to figure out which
    /// verifier set was the one that actually performed the validation.
    pub signing_verifier_set_hash: [u8; 32],
}

impl SignatureVerification {
    pub fn is_valid(&self) -> bool {
        self.accumulated_threshold == U128::MAX
    }
}

#[cfg(test)]
mod tests {
    use core::mem::size_of;

    use super::*;

    #[test]
    fn initialization() {
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
    fn serialization() {
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
    fn alignment_compatibility() {
        // Critical: alignment must be ≤ 8 to work with Anchor's 8-byte discriminator
        // The struct data starts at byte offset 8, which is 8-byte aligned but NOT 16-byte aligned
        let alignment = std::mem::align_of::<SignatureVerificationSessionData>();
        assert!(
               alignment <= 8,
               "Struct alignment ({alignment}) must be <= 8 bytes for zero-copy compatibility with Anchor discriminator",
           );
    }
}
