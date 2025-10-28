use anchor_lang::prelude::*;

#[error_code]
pub enum ITSError {
    InvalidArgument,
    Paused,
    InvalidInstructionData,
    InvalidAccountOwner,
    DeployApprovalPDANotProvided,
    MinterNotProvided,
    InvalidRole,
    InvalidAccountData,
    #[msg("The token mint cannot have fixed zero supply")]
    ZeroSupplyToken,
    #[msg("The mint extension is not compatible with the TokenManager type")]
    TokenManagerMintExtensionMismatch,
}
