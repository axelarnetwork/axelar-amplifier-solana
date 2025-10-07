use crate::{GovernanceConfig, GovernanceError, Hash};
use anchor_lang::prelude::*;
use axelar_solana_governance::seed_prefixes;

#[derive(Accounts)]
pub struct UpdateConfigAccounts<'info> {
    #[account(mut, constraint = governance_config.load()?.operator == payer.key().to_bytes() @ GovernanceError::NotOperator)]
    pub payer: Signer<'info>,
    #[account(
            mut,
            seeds = [seed_prefixes::GOVERNANCE_CONFIG],
            bump
        )]
    pub governance_config: AccountLoader<'info, GovernanceConfig>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct GovernanceConfigUpdate {
    pub chain_hash: Option<Hash>,
    pub address_hash: Option<Hash>,
    pub minimum_proposal_eta_delay: Option<u32>,
}

pub fn update_config_handler(
    ctx: Context<UpdateConfigAccounts>,
    params: GovernanceConfigUpdate,
) -> Result<()> {
    let config = &mut ctx.accounts.governance_config;
    config.load_mut()?.update(params)
}
