use anchor_lang::prelude::*;

#[error_code]
pub enum ItsError {
    #[msg("The ITS program is paused")]
    Paused,
    InvalidArgument,
    InvalidInstructionData,
    InvalidAccountOwner,
    DeployApprovalPDANotProvided,
    MinterNotProvided,
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
    #[msg("The token mint cannot have fixed zero supply")]
    ZeroSupplyToken,
    #[msg("The mint extension is not compatible with the TokenManager type")]
    TokenManagerMintExtensionMismatch,
    TokenMintMismatch,
    InvalidTokenManagerAta,
    InvalidTokenManagerPda,
}
