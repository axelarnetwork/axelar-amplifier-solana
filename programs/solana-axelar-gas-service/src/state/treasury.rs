use anchor_lang::prelude::*;

#[account(zero_copy)]
#[derive(InitSpace, PartialEq, Eq, Debug)]
#[allow(clippy::partial_pub_fields)]
pub struct Treasury {
    /// The bump seed used to derive the PDA, ensuring the address is valid.
    pub bump: u8,
}

impl Treasury {
    pub const SEED_PREFIX: &'static [u8] = b"gas-service";

    pub fn pda_seeds<'a>() -> [&'a [u8]; 1] {
        [Self::SEED_PREFIX]
    }

    pub fn try_find_pda() -> Option<(Pubkey, u8)> {
        Pubkey::try_find_program_address(&Self::pda_seeds(), &crate::ID)
    }

    pub fn find_pda() -> (Pubkey, u8) {
        Pubkey::find_program_address(&Self::pda_seeds(), &crate::ID)
    }
}
