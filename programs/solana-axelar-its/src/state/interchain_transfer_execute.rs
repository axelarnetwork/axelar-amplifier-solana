use anchor_lang::prelude::*;

/// Signing PDA to prove/verify that ITS called an executable.
pub struct InterchainTransferExecute {}

impl InterchainTransferExecute {
    pub const SEED_PREFIX: &'static [u8] = b"interchain-transfer-execute";

    fn pda_seeds<'a>(destination_program: &'a Pubkey) -> [&'a [u8]; 2] {
        [Self::SEED_PREFIX, destination_program.as_ref()]
    }

    pub fn try_find_pda(destination_program: &Pubkey) -> Option<(Pubkey, u8)> {
        Pubkey::try_find_program_address(&Self::pda_seeds(destination_program)[..], &crate::ID)
    }

    pub fn find_pda(destination_program: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&Self::pda_seeds(destination_program)[..], &crate::ID)
    }
}
