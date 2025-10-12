use crate::{seed_prefixes::GOVERNANCE_CONFIG, GovernanceConfig, GovernanceError};
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct InitializeConfig<'info> {
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
            space = GovernanceConfig::DISCRIMINATOR.len() + std::mem::size_of::<GovernanceConfig>(),
            seeds = [GOVERNANCE_CONFIG],
            bump
        )]
    pub governance_config: AccountLoader<'info, GovernanceConfig>,
    pub system_program: Program<'info, System>,
}

pub fn initialize_config_handler(
    ctx: Context<InitializeConfig>,
    params: GovernanceConfig,
) -> Result<()> {
    msg!("initialize_config_handler");

    // Validate the config
    params.validate_config()?;

    let config = &mut ctx.accounts.governance_config.load_init()?;

    // Initialize account data
    config.bump = ctx.bumps.governance_config;
    config.chain_hash = params.chain_hash;
    config.address_hash = params.address_hash;
    config.minimum_proposal_eta_delay = params.minimum_proposal_eta_delay;
    config.operator = params.operator;

    Ok(())
}
