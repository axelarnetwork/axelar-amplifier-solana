use anchor_lang::prelude::*;

/// PDA signed by the programs to authorize a
/// call contract on the gateway.
pub struct CallContractSigner {}

impl CallContractSigner {
    pub const SEED_PREFIX: &'static [u8] = b"gtw-call-contract";

    fn pda_seeds() -> [&'static [u8]; 1] {
        [Self::SEED_PREFIX]
    }

    pub fn try_find_pda(program_id: &Pubkey) -> Option<(Pubkey, u8)> {
        Pubkey::try_find_program_address(&Self::pda_seeds()[..], program_id)
    }

    pub fn find_pda(program_id: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&Self::pda_seeds()[..], program_id)
    }

    pub fn create_pda(bump: u8, program_id: &Pubkey) -> Result<Pubkey> {
        Pubkey::create_program_address(&[Self::SEED_PREFIX, &[bump]], program_id)
            .map_err(|_| crate::GatewayError::InvalidSigningPDABump.into())
    }
}
