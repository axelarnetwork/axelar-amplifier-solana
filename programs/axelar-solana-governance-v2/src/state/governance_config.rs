use crate::{GovernanceConfigUpdate, GovernanceError};
use anchor_lang::prelude::*;
use axelar_solana_governance::state::VALID_PROPOSAL_DELAY_RANGE;

pub type Hash = [u8; 32];
/// The [`solana_program::pubkey::Pubkey`] bytes.
pub type Address = [u8; 32];

/// Governance configuration type.
#[account(zero_copy)]
#[derive(Debug, AnchorSerialize, AnchorDeserialize)]
pub struct GovernanceConfig {
    /// The bump for this account.
    pub bump: u8,
    pub _padding: [u8; 7],
    /// The name hash of the governance chain of the remote GMP contract. This
    /// param is used for validating the incoming GMP governance message.
    pub chain_hash: Hash,
    /// The address hash of the remote GMP governance contract. This param
    /// is used for validating the incoming GMP governance message.
    pub address_hash: Hash,
    /// This is the minimum time in seconds from `now()` a proposal can
    /// be executed. If the incoming GMP proposal does not have an ETA
    /// superior to `unix_timestamp` + `this field`, such ETA will be
    /// will be set as new ETA.
    pub minimum_proposal_eta_delay: u32,
    /// The pub key of the operator. This address is able to execute proposals
    /// that were previously scheduled by the Axelar governance infrastructure
    /// via GMP flow regardless of the proposal ETA.
    pub operator: Address,
}

impl GovernanceConfig {
    /// Creates a new governance program config.
    #[must_use]
    pub const fn new(
        chain_hash: Hash,
        address_hash: Hash,
        minimum_proposal_eta_delay: u32,
        operator: Address,
    ) -> Self {
        Self {
            bump: 0, // This will be set by the program
            _padding: [0u8; 7],
            chain_hash,
            address_hash,
            minimum_proposal_eta_delay,
            operator,
        }
    }

    pub fn validate_config(&self) -> Result<()> {
        if !VALID_PROPOSAL_DELAY_RANGE.contains(&self.minimum_proposal_eta_delay) {
            msg!(
                "The minimum proposal ETA delay must be among {} and {} seconds",
                VALID_PROPOSAL_DELAY_RANGE.start(),
                VALID_PROPOSAL_DELAY_RANGE.end()
            );
            return err!(GovernanceError::InvalidArgument);
        }
        Ok(())
    }

    pub fn update(&mut self, mut update: GovernanceConfigUpdate) -> Result<()> {
        if let Some(new_chain_hash) = update.chain_hash.take() {
            self.chain_hash = new_chain_hash;
        }

        if let Some(new_address_hash) = update.address_hash.take() {
            self.address_hash = new_address_hash;
        }

        if let Some(new_minimum_proposal_eta_delay) = update.minimum_proposal_eta_delay.take() {
            self.minimum_proposal_eta_delay = new_minimum_proposal_eta_delay;
        }
        self.validate_config()
    }
}
