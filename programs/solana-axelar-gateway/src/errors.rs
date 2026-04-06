use anchor_lang::prelude::*;

#[error_code]
pub enum GatewayError {
    VerifierSetTooOld,
    EpochCalculationOverflow,
    InvalidDomainSeparator,
    SignatureVerificationFailed,
    InvalidMerkleProof,
    SlotIsOutOfBounds,
    SlotAlreadyVerified,
    InvalidDigitalSignature,
    LeafNodeNotPartOfMerkleRoot,
    InvalidVerificationSessionPDA,
    SigningSessionNotValid,
    InvalidDestinationAddress,
    InvalidVerifierSetTrackerPDA,
    MessageNotApproved,
    InvalidMessageHash,
    InvalidSigningPDA,
    ProofNotSignedByLatestVerifierSet,
    RotationCooldownNotDone,
    DuplicateVerifierSetRotation,
    InvalidVerifierSetTrackerProvided,
    InvalidUpgradeAuthority,
    InvalidLoaderContent,
    InvalidLoaderState,
    InvalidOperatorOrAuthorityAccount,
    CallerNotSigner,
    UnsupportedSignatureScheme,
    InvalidSigningPDABump,
    InvalidTimestamp,
    #[msg("Invalid encoding scheme")]
    InvalidEncodingScheme,
    #[msg("Borsh serialize error")]
    BorshSerializeError,
    #[msg("Borsh deserialize error")]
    BorshDeserializeError,
    #[msg("ABI error")]
    PayloadAbiError,
    #[msg("Internal type conversion error")]
    PayloadConversion,
}
