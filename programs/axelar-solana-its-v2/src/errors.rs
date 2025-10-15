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
}
