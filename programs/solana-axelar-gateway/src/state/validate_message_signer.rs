use anchor_lang::prelude::*;

/// PDA signed by the programs to validate incoming
/// messages on the gateway.
pub struct ValidateMessageSigner {}

impl ValidateMessageSigner {
    pub const SEED_PREFIX: &'static [u8] = b"gtw-validate-msg";

    fn pda_seeds(command_id: &[u8; 32]) -> [&[u8]; 2] {
        [Self::SEED_PREFIX, command_id.as_ref()]
    }

    pub fn try_find_pda(command_id: &[u8; 32], program_id: &Pubkey) -> Option<(Pubkey, u8)> {
        Pubkey::try_find_program_address(&Self::pda_seeds(command_id)[..], program_id)
    }

    pub fn find_pda(command_id: &[u8; 32], program_id: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&Self::pda_seeds(command_id)[..], program_id)
    }

    pub fn create_pda(command_id: &[u8; 32], bump: u8, program_id: &Pubkey) -> Result<Pubkey> {
        Pubkey::create_program_address(
            &[Self::SEED_PREFIX, command_id.as_ref(), &[bump]],
            program_id,
        )
        .map_err(|_| crate::GatewayError::InvalidSigningPDABump.into())
    }
}
