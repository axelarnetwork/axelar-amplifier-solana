use crate::seed_prefixes::VALIDATE_MESSAGE_SIGNING_SEED;
use anchor_lang::prelude::*;
use bytemuck::{Pod, Zeroable};

#[account(zero_copy)]
#[derive(Debug, PartialEq, Eq)]
#[allow(clippy::pub_underscore_fields)]
pub struct IncomingMessage {
    pub bump: u8,
    pub signing_pda_bump: u8,
    pub _pad: [u8; 3],
    pub status: MessageStatus,
    pub message_hash: [u8; 32],
    pub payload_hash: [u8; 32],
}

impl IncomingMessage {
    pub const SEED_PREFIX: &'static [u8] = b"incoming message";
    pub const VALIDATE_MESSAGE_SEED_PREFIX: &'static [u8] = b"gtw-validate-msg";

    pub fn pda_seeds<'a>(command_id: &'a [u8; 32]) -> [&'a [u8]; 2] {
        [Self::SEED_PREFIX, command_id]
    }

    pub fn try_find_pda(command_id: &[u8; 32]) -> Option<(Pubkey, u8)> {
        Pubkey::try_find_program_address(&Self::pda_seeds(command_id), &crate::ID)
    }

    pub fn find_pda(command_id: &[u8; 32]) -> (Pubkey, u8) {
        Pubkey::find_program_address(&Self::pda_seeds(command_id), &crate::ID)
    }

    pub fn find_signing_pda(command_id: &[u8; 32], destination_address: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[VALIDATE_MESSAGE_SIGNING_SEED, command_id],
            destination_address,
        )
    }
}

#[repr(C)]
#[derive(Copy, Clone, PartialEq, Eq, Debug, Pod, Zeroable, AnchorSerialize, AnchorDeserialize)]
pub struct MessageStatus(u8);

impl MessageStatus {
    /// Creates a `MessageStatus` value which can be interpreted as "approved".
    #[must_use]
    pub const fn approved() -> Self {
        Self(0)
    }

    #[must_use]
    pub const fn executed() -> Self {
        Self(1)
    }

    #[must_use]
    pub const fn is_approved(&self) -> bool {
        self.0 == 0
    }

    #[must_use]
    pub const fn is_executed(&self) -> bool {
        self.0 != 0
    }
}
