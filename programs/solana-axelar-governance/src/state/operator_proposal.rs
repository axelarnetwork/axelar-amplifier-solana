use anchor_lang::prelude::*;

#[derive(Debug, InitSpace)]
#[account]
pub struct OperatorProposal {}

impl OperatorProposal {
    pub const SEED_PREFIX: &'static [u8] = b"operator-managed-proposal";

    pub fn pda_seeds(proposal_hash: &[u8; 32]) -> [&[u8]; 2] {
        [Self::SEED_PREFIX, proposal_hash]
    }

    pub fn try_find_pda(proposal_hash: &[u8; 32]) -> Option<(Pubkey, u8)> {
        Pubkey::try_find_program_address(&Self::pda_seeds(proposal_hash), &crate::ID)
    }

    pub fn find_pda(proposal_hash: &[u8; 32]) -> (Pubkey, u8) {
        Pubkey::find_program_address(&Self::pda_seeds(proposal_hash), &crate::ID)
    }
}
