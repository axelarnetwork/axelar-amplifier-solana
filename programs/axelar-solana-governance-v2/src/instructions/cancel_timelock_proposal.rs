use crate::{ExecutableProposal, GovernanceConfig, ProposalCancelled};
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[event_cpi]
#[instruction(proposal_hash: [u8; 32], eta: u64, native_value: Vec<u8>, target: Vec<u8>, call_data: Vec<u8>)]
pub struct CancelTimelockProposal<'info> {
    #[account(
            seeds = [axelar_solana_governance::seed_prefixes::GOVERNANCE_CONFIG],
            bump = governance_config.bump,
        )]
    pub governance_config: Account<'info, GovernanceConfig>,
    #[account(
            mut,
            close = governance_config,
            seeds = [axelar_solana_governance::seed_prefixes::PROPOSAL_PDA, &proposal_hash],
            bump = proposal_pda.bump
        )]
    pub proposal_pda: Account<'info, ExecutableProposal>,
}

pub fn cancel_timelock_proposal_instruction_handler(
    ctx: Context<CancelTimelockProposal>,
    proposal_hash: [u8; 32],
    eta: u64,
    native_value: Vec<u8>,
    target: Vec<u8>,
    call_data: Vec<u8>,
) -> Result<()> {
    emit_cpi!(ProposalCancelled {
        hash: proposal_hash,
        target_address: target,
        call_data,
        native_value,
        eta,
    });

    Ok(())
}
