use crate::state::user_roles::Roles;
use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace, PartialEq, Eq, Copy, Debug)]
/// Proposal to transfer roles to a user.
pub struct RoleProposal {
    // The roles to be transferred.
    pub roles: Roles,
    /// The bump seed used to derive the PDA, ensuring the address is valid.
    pub bump: u8,
}

impl RoleProposal {
    pub const SEED_PREFIX: &'static [u8] = b"role-proposal";

    fn pda_seeds<'a>(resource: &'a Pubkey, origin: &'a Pubkey, user: &'a Pubkey) -> [&'a [u8]; 4] {
        [
            Self::SEED_PREFIX,
            resource.as_ref(),
            origin.as_ref(),
            user.as_ref(),
        ]
    }

    pub fn find_pda(
        resource: &Pubkey,
        origin: &Pubkey,
        new_operator: &Pubkey,
        program_id: &Pubkey,
    ) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &RoleProposal::pda_seeds(resource, origin, &new_operator)[..],
            program_id,
        )
    }
}
