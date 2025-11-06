use anchor_lang::prelude::*;

#[error_code]
pub enum ItsError {
    #[msg("The ITS program is paused")]
    Paused,
    InvalidArgument,
    InvalidInstructionData,
    InvalidAccountOwner,
    MinterNotProvided,
    MinterRolesNotProvided,
    MinterRolesPdaBumpNotProvided,
    OperatorNotProvided,
    OperatorRolesPdaNotProvided,
    InvalidAccountData,
    MissingRemainingAccount,
    #[msg("The role provided is invalid")]
    InvalidRole,
    #[msg("The token manager type is invalid")]
    InvalidTokenManagerType,
    #[msg("The source chain name is untrusted")]
    UntrustedSourceChain,
    #[msg("The destination chain name is untrusted")]
    UntrustedDestinationChain,
    #[msg("The destination chain name is invalid")]
    InvalidDestinationChain,
    #[msg("The destination address account is invalid")]
    InvalidDestinationAddressAccount,
    #[msg("The token mint cannot have fixed zero supply")]
    ZeroSupplyToken,
    #[msg("The mint extension is not compatible with the TokenManager type")]
    TokenManagerMintExtensionMismatch,
    #[msg("The signer is not a user account")]
    CallerNotUserAccount,
    #[msg("The token mint is invalid")]
    TokenMintMismatch,
    InvalidTokenManagerAta,
    InvalidTokenManagerPda,
    AccountNotProvided,
    SourceIdNotProvided,
    PdaSeedsNotProvided,
    #[msg("source_id and pda_seeds must both be provided together or both be None")]
    InconsistentSourceIdAndPdaSeeds,
}
