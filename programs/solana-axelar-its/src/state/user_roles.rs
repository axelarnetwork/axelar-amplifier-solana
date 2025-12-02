use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace, PartialEq, Eq, Copy, Debug)]
pub struct UserRoles {
    pub roles: u8,
    /// The bump seed used to derive the PDA, ensuring the address is valid.
    pub bump: u8,
}

impl UserRoles {
    /// The seeds for the PDA are:
    /// - SEED_PREFIX
    /// - ITS Root PDA key
    /// - User key
    pub const SEED_PREFIX: &'static [u8] = b"user-roles";

    pub fn pda_seeds<'a>(resource: &'a Pubkey, user: &'a Pubkey) -> [&'a [u8]; 3] {
        [Self::SEED_PREFIX, resource.as_ref(), user.as_ref()]
    }

    pub fn has_minter_role(&self) -> bool {
        let res = self.roles & roles::MINTER;
        return res == roles::MINTER;
    }

    pub fn has_operator_role(&self) -> bool {
        let res = self.roles & roles::OPERATOR;
        return res == roles::OPERATOR;
    }

    pub fn has_flow_limiter_role(&self) -> bool {
        let res = self.roles & roles::FLOW_LIMITER;
        return res == roles::FLOW_LIMITER;
    }

    pub fn has_roles(&self) -> bool {
        self.roles != roles::EMPTY
    }

    pub fn contains(&self, role: u8) -> bool {
        let res = self.roles & role;
        return res == role;
    }

    pub fn insert(&mut self, new_role: u8) {
        self.roles |= new_role;
    }

    pub fn remove(&mut self, role: u8) {
        self.roles &= !role;
    }

    pub fn find_pda(resource: &Pubkey, user: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[UserRoles::SEED_PREFIX, resource.as_ref(), user.as_ref()],
            &crate::ID,
        )
    }
}

/// Roles that can be assigned to a user.
pub mod roles {
    /// Can mint new tokens.
    pub const MINTER: u8 = 0b0000_0001;

    /// Can perform operations on the resource.
    pub const OPERATOR: u8 = 0b0000_0010;

    /// Can change the limit to the flow of tokens.
    pub const FLOW_LIMITER: u8 = 0b0000_0100;

    pub const EMPTY: u8 = 0b0000_0000;
}

#[error_code]
pub enum RolesError {
    #[msg("User does not have the MINTER role.")]
    MissingMinterRole,
    #[msg("User does not have the OPERATOR role.")]
    MissingOperatorRole,
    #[msg("User does not have the FLOW_LIMITER role.")]
    MissingFlowLimiterRole,

    #[msg("Proposal does not have the MINTER role.")]
    ProposalMissingMinterRole,
    #[msg("Proposal does not have the OPERATOR role.")]
    ProposalMissingOperatorRole,
    #[msg("Proposal does not have the FLOW_LIMITER role.")]
    ProposalMissingFlowLimiterRole,
}

#[cfg(test)]
mod tests {
    use borsh::to_vec;

    use super::*;

    #[test]
    fn user_roles_round_trip() {
        let original = UserRoles {
            roles: roles::MINTER | roles::OPERATOR,
            bump: 42,
        };

        let serialized = to_vec(&original).unwrap();
        let deserialized = UserRoles::try_from_slice(&serialized).unwrap();

        assert_eq!(original, deserialized);
        assert!(original.contains(roles::MINTER));
        assert!(original.contains(roles::OPERATOR));
        assert!(deserialized.contains(roles::MINTER | roles::OPERATOR));
    }

    #[test]
    fn roles_bitflags() {
        let roles_list = vec![
            roles::MINTER,
            roles::OPERATOR,
            roles::FLOW_LIMITER,
            roles::MINTER | roles::OPERATOR,
            roles::OPERATOR | roles::FLOW_LIMITER,
            roles::MINTER | roles::FLOW_LIMITER,
            roles::MINTER | roles::OPERATOR | roles::FLOW_LIMITER,
        ];

        for roles in roles_list {
            let original = UserRoles { roles, bump: 0 };

            let serialized = to_vec(&original).unwrap();
            let deserialized = UserRoles::try_from_slice(&serialized).unwrap();

            assert_eq!(original, deserialized);
        }
    }
}
