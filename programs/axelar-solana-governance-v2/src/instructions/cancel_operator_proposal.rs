use crate::{
    seed_prefixes::{GOVERNANCE_CONFIG, OPERATOR_MANAGED_PROPOSAL, PROPOSAL_PDA},
    ExecutableProposal, GovernanceConfig, OperatorProposal, OperatorProposalCancelled,
};
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[event_cpi]
#[instruction(proposal_hash: [u8; 32], native_value: Vec<u8>, target: Vec<u8>, call_data: Vec<u8>)]
pub struct CancelOperatorProposal<'info> {
    #[account(
            signer,
            seeds = [GOVERNANCE_CONFIG],
            bump = governance_config.load()?.bump,
        )]
    pub governance_config: AccountLoader<'info, GovernanceConfig>,
    #[account(
            seeds = [PROPOSAL_PDA, &proposal_hash],
            bump = proposal_pda.load()?.bump
        )]
    pub proposal_pda: AccountLoader<'info, ExecutableProposal>,
    #[account(
            mut,
            close = governance_config,
            seeds = [OPERATOR_MANAGED_PROPOSAL, &proposal_hash],
            bump = proposal_pda.load()?.managed_bump
        )]
    pub operator_proposal_pda: AccountLoader<'info, OperatorProposal>,
}

pub fn cancel_operator_proposal_handler(
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
