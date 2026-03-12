use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("Only the owner can perform this action")]
    UnauthorizedOwner,
    #[msg("New owner cannot be the same as current owner")]
    SameOwner,
}
