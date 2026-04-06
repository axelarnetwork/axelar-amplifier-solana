use anchor_lang::prelude::*;

#[error_code]
pub enum GatewayError {
    #[msg("Verifier set is too old")]
    VerifierSetTooOld,
    #[msg("Epoch calculation overflow")]
    EpochCalculationOverflow,
    #[msg("Invalid domain separator")]
    InvalidDomainSeparator,
    #[msg("Signature verification failed")]
    SignatureVerificationFailed,
    #[msg("Invalid Merkle proof")]
    InvalidMerkleProof,
    #[msg("Slot is out of bounds")]
    SlotIsOutOfBounds,
    #[msg("Slot already verified")]
    SlotAlreadyVerified,
    #[msg("Invalid digital signature")]
    InvalidDigitalSignature,
    #[msg("Leaf node not part of Merkle root")]
    LeafNodeNotPartOfMerkleRoot,
    #[msg("Invalid verification session PDA")]
    InvalidVerificationSessionPDA,
    #[msg("Signing session not valid")]
    SigningSessionNotValid,
    #[msg("Invalid destination address")]
    InvalidDestinationAddress,
    #[msg("Invalid verifier set tracker PDA")]
    InvalidVerifierSetTrackerPDA,
    #[msg("Message not approved")]
    MessageNotApproved,
    #[msg("Invalid message hash")]
    InvalidMessageHash,
    #[msg("Invalid signing PDA")]
    InvalidSigningPDA,
    #[msg("Proof not signed by latest verifier set")]
    ProofNotSignedByLatestVerifierSet,
    #[msg("Rotation cooldown not done")]
    RotationCooldownNotDone,
    #[msg("Duplicate verifier set rotation")]
    DuplicateVerifierSetRotation,
    #[msg("Invalid verifier set tracker provided")]
    InvalidVerifierSetTrackerProvided,
    #[msg("Invalid upgrade authority")]
    InvalidUpgradeAuthority,
    #[msg("Invalid loader content")]
    InvalidLoaderContent,
    #[msg("Invalid loader state")]
    InvalidLoaderState,
    #[msg("Invalid operator or authority account")]
    InvalidOperatorOrAuthorityAccount,
    #[msg("Caller not signer")]
    CallerNotSigner,
    #[msg("Unsupported signature scheme")]
    UnsupportedSignatureScheme,
    #[msg("Invalid signing PDA bump")]
    InvalidSigningPDABump,
    #[msg("Invalid timestamp")]
    InvalidTimestamp,
    #[msg("Invalid encoding scheme")]
    InvalidEncodingScheme,
    #[msg("Borsh serialize error")]
    BorshSerializeError,
    #[msg("Borsh deserialize error")]
    BorshDeserializeError,
    #[msg("ABI error")]
    PayloadAbiError,
}
