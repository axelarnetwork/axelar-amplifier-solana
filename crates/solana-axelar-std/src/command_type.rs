use anchor_lang::prelude::*;

#[repr(u8)]
#[derive(AnchorSerialize, Clone, Copy, Debug, Eq, PartialEq, AnchorDeserialize)]
pub enum CommandType {
    ApproveMessages = 0,
    RotateSigners = 1,
}
