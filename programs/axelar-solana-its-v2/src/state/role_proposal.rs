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

    pub fn pda_seeds<'a>(resource: &'a Pubkey, user: &'a Pubkey) -> [&'a [u8]; 3] {
        [Self::SEED_PREFIX, resource.as_ref(), user.as_ref()]
    }
}
