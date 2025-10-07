use crate::{ExecutableProposal, GovernanceConfig, OperatorProposal, OperatorProposalCancelled};
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[event_cpi]
#[instruction(proposal_hash: [u8; 32], native_value: Vec<u8>, target: Vec<u8>, call_data: Vec<u8>)]
pub struct CancelOperatorProposal<'info> {
    #[account(
            seeds = [axelar_solana_governance::seed_prefixes::GOVERNANCE_CONFIG],
            bump = governance_config.load()?.bump,
        )]
    pub governance_config: AccountLoader<'info, GovernanceConfig>,
    #[account(
            seeds = [axelar_solana_governance::seed_prefixes::PROPOSAL_PDA, &proposal_hash],
            bump = proposal_pda.bump
        )]
    pub proposal_pda: Account<'info, ExecutableProposal>,
    #[account(
            mut,
            close = governance_config,
            seeds = [axelar_solana_governance::seed_prefixes::OPERATOR_MANAGED_PROPOSAL, &proposal_hash],
            bump = proposal_pda.managed_bump
        )]
    pub operator_proposal_pda: Account<'info, OperatorProposal>,
}

pub fn cancel_operator_proposal_instruction_handler(
    ctx: Context<CancelOperatorProposal>,
    proposal_hash: [u8; 32],
    native_value: Vec<u8>,
    target: Vec<u8>,
    call_data: Vec<u8>,
) -> Result<()> {
    emit_cpi!(OperatorProposalCancelled {
        hash: proposal_hash,
        target_address: target,
        call_data,
        native_value,
    });

    Ok(())
}
