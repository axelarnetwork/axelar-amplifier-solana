use anchor_lang::prelude::*;

#[account(zero_copy)]
#[derive(InitSpace, PartialEq, Eq, Debug)]
#[allow(clippy::partial_pub_fields)]
pub struct Treasury {
    #[doc(hidden)]
    _old_operator: [u8; 32],
    /// The bump seed used to derive the PDA, ensuring the address is valid.
    pub bump: u8,
}

impl Treasury {
    pub const SEED_PREFIX: &'static [u8] = b"gas-service";

    pub fn find_pda() -> (Pubkey, u8) {
        Pubkey::find_program_address(&[Self::SEED_PREFIX], &crate::ID)
    }
}
