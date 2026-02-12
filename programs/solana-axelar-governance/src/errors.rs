use anchor_lang::prelude::*;

#[error_code]
pub enum GovernanceError {
    InvalidUpgradeAuthority,
    InvalidArgument,
    NotOperator,
    ArithmeticOverflow,
    UnauthorizedChain,
    UnauthorizedAddress,
    InvalidInstructionData,
    ProposalNotReady,
    InvalidTargetProgram,
    TargetAccountNotFound,
    MissingNativeValueReceiver,
    InvalidNativeValue,
    UnauthorizedOperator,
    MissingRequiredSignature,
    GovernanceConfigMissing,
}
