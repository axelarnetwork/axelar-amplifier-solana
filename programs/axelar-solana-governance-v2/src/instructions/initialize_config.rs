use crate::{GovernanceConfig, GovernanceError};
use anchor_lang::prelude::*;
use axelar_solana_governance::seed_prefixes;

#[derive(Accounts)]
pub struct InitializeConfigAccounts<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub upgrade_authority: Signer<'info>,
    #[account(
            seeds = [crate::ID.as_ref()],
            bump,
            seeds::program = anchor_lang::solana_program::bpf_loader_upgradeable::ID,
            constraint = program_data.upgrade_authority_address == Some(upgrade_authority.key()) @ GovernanceError::InvalidUpgradeAuthority
        )]
    pub program_data: Account<'info, ProgramData>,
    #[account(
            init,
            payer = payer,
            space = 8 + std::mem::size_of::<GovernanceConfig>(),
            seeds = [seed_prefixes::GOVERNANCE_CONFIG],
            bump
        )]
    pub governance_config: Account<'info, GovernanceConfig>,
    pub system_program: Program<'info, System>,
}

pub fn initialize_config_handler(
    ctx: Context<InitializeConfigAccounts>,
    config: GovernanceConfig,
) -> Result<()> {
    config.validate_config()?;

    // Initialize account data
    ctx.accounts.governance_config.bump = ctx.bumps.governance_config;

    ctx.accounts.governance_config.chain_hash = config.chain_hash;
    ctx.accounts.governance_config.address_hash = config.address_hash;
    ctx.accounts.governance_config.minimum_proposal_eta_delay = config.minimum_proposal_eta_delay;
    ctx.accounts.governance_config.operator = config.operator;

    Ok(())
}
