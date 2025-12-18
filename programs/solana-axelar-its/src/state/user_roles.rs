use anchor_lang::prelude::*;
use bitflags::bitflags;

#[account]
#[derive(InitSpace, PartialEq, Eq, Copy, Debug)]
pub struct UserRoles {
    pub roles: Roles,
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

    pub fn try_find_pda(resource: &Pubkey, user: &Pubkey) -> Option<(Pubkey, u8)> {
        Pubkey::try_find_program_address(&Self::pda_seeds(resource, user), &crate::ID)
    }

    pub fn has_minter_role(&self) -> bool {
        self.roles.contains(Roles::MINTER)
    }

    pub fn has_operator_role(&self) -> bool {
        self.roles.contains(Roles::OPERATOR)
    }

    pub fn has_flow_limiter_role(&self) -> bool {
        self.roles.contains(Roles::FLOW_LIMITER)
    }

    pub fn has_roles(&self) -> bool {
        self.roles.bits() != 0u8
    }

    pub fn find_pda(resource: &Pubkey, user: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[UserRoles::SEED_PREFIX, resource.as_ref(), user.as_ref()],
            &crate::ID,
        )
    }
}

// Roles flag used in ITS

bitflags! {
    /// Roles that can be assigned to a user.
    #[derive(Debug, Eq, PartialEq, Clone, Copy)]
    pub struct Roles: u8 {
        /// Can mint new tokens.
        const MINTER = 0b0000_0001;

        /// Can perform operations on the resource.
        const OPERATOR = 0b0000_0010;

        /// Can change the limit to the flow of tokens.
        const FLOW_LIMITER = 0b0000_0100;
    }
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

impl anchor_lang::Space for Roles {
    const INIT_SPACE: usize = 1;
}

impl PartialEq<u8> for Roles {
    fn eq(&self, other: &u8) -> bool {
        self.bits().eq(other)
    }
}

impl PartialEq<Roles> for u8 {
    fn eq(&self, other: &Roles) -> bool {
        self.eq(&other.bits())
    }
}

impl AnchorSerialize for Roles {
    fn serialize<W: std::io::prelude::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        self.bits().serialize(writer)
    }
}

impl AnchorDeserialize for Roles {
    fn deserialize_reader<R: std::io::prelude::Read>(reader: &mut R) -> std::io::Result<Self> {
        let byte = u8::deserialize_reader(reader)?;
        Ok(Self::from_bits_truncate(byte))
    }
}

impl From<RolesError> for ProgramError {
    fn from(val: RolesError) -> Self {
        anchor_lang::error::Error::from(val).into()
    }
}

#[cfg(test)]
mod tests {
    use borsh::to_vec;

    use super::*;

    #[test]
    fn user_roles_round_trip() {
        let original = UserRoles {
            roles: Roles::MINTER | Roles::OPERATOR,
            bump: 42,
        };

        let serialized = to_vec(&original).unwrap();
        let deserialized = UserRoles::try_from_slice(&serialized).unwrap();

        assert_eq!(original, deserialized);
        assert!(original.roles.contains(Roles::MINTER));
        assert!(original.roles.contains(Roles::OPERATOR));
        assert!(deserialized.roles.contains(Roles::MINTER | Roles::OPERATOR));
    }

    #[test]
    fn roles_bitflags() {
        let roles_list = vec![
            Roles::MINTER,
            Roles::OPERATOR,
            Roles::FLOW_LIMITER,
            Roles::MINTER | Roles::OPERATOR,
            Roles::OPERATOR | Roles::FLOW_LIMITER,
            Roles::MINTER | Roles::FLOW_LIMITER,
            Roles::MINTER | Roles::OPERATOR | Roles::FLOW_LIMITER,
        ];

        for roles in roles_list {
            let original = UserRoles { roles, bump: 0 };

            let serialized = to_vec(&original).unwrap();
            let deserialized = UserRoles::try_from_slice(&serialized).unwrap();

            assert_eq!(original, deserialized);
        }
    }
}
