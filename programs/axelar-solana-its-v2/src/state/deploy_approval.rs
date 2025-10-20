use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace, Debug)]
pub struct DeployApproval {
    /// Hash of the approved destination minter
    pub approved_destination_minter: [u8; 32],
    /// PDA bump seed
    pub bump: u8,
}
