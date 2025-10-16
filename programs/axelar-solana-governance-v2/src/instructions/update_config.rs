use crate::{GovernanceConfig, GovernanceError, Hash};
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct UpdateConfig<'info> {
    #[account(
        mut,
        constraint = governance_config.operator == payer.key().to_bytes()
            @ GovernanceError::NotOperator
    )]
    pub payer: Signer<'info>,

    #[account(
        mut,
        seeds = [GovernanceConfig::SEED_PREFIX],
        bump
    )]
    pub governance_config: Account<'info, GovernanceConfig>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct GovernanceConfigUpdate {
    pub chain_hash: Option<Hash>,
    pub address_hash: Option<Hash>,
    pub minimum_proposal_eta_delay: Option<u32>,
}

pub fn update_config_handler(
    ctx: Context<UpdateConfig>,
    params: GovernanceConfigUpdate,
) -> Result<()> {
    let config = &mut ctx.accounts.governance_config;
    config.update(params)
}
