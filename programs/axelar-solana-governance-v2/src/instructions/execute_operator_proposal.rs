use crate::{
    check_governance_config_presence, check_target_program_presence, execute_proposal_cpi,
    ExecutableProposal, ExecuteProposalData, GovernanceConfig, GovernanceError,
    OperatorProposalExecuted,
};
use anchor_lang::prelude::*;
use axelar_solana_governance::seed_prefixes;

#[derive(Accounts)]
#[event_cpi]
#[instruction(execute_proposal_data: ExecuteProposalData)]
pub struct ExecuteOperatorProposal<'info> {
    pub system_program: Program<'info, System>,
    #[account(
        seeds = [seed_prefixes::GOVERNANCE_CONFIG],
        bump = governance_config.load()?.bump,
    )]
    pub governance_config: AccountLoader<'info, GovernanceConfig>,
    #[account(
        mut,
        close = governance_config,
        seeds = [
            seed_prefixes::PROPOSAL_PDA,
            &{
                ExecutableProposal::calculate_hash(
                    &Pubkey::new_from_array(execute_proposal_data.target_address),
                    &execute_proposal_data.call_data,
                    &execute_proposal_data.native_value,
                )
            }
        ],
        bump = proposal_pda.bump
    )]
    pub proposal_pda: Account<'info, crate::ExecutableProposal>,
    /// The operator account that must sign this transaction
    #[account(
        constraint = operator.key().to_bytes() == governance_config.load()?.operator @ GovernanceError::UnauthorizedOperator
    )]
    pub operator: Signer<'info>,
    #[account(
        mut,
        close = governance_config,
        seeds = [
            seed_prefixes::OPERATOR_MANAGED_PROPOSAL,
            &{
                ExecutableProposal::calculate_hash(
                    &Pubkey::new_from_array(execute_proposal_data.target_address),
                    &execute_proposal_data.call_data,
                    &execute_proposal_data.native_value,
                )
            }
        ],
        bump
    )]
    pub operator_pda_marker_account: Account<'info, crate::OperatorProposal>,
}

pub fn execute_operator_proposal_handler(
    ctx: Context<ExecuteOperatorProposal>,
    execute_proposal_data: ExecuteProposalData,
) -> Result<()> {
    let target_program = Pubkey::new_from_array(execute_proposal_data.target_address);

    // Note: No ETA validation for operator proposals - they can be executed immediately
    let remaining_accounts = ctx.remaining_accounts;
    let governance_config = ctx.accounts.governance_config.clone();

    check_governance_config_presence(
        &ctx.accounts.governance_config.key(),
        remaining_accounts,
        &execute_proposal_data.call_data.solana_accounts,
    )?;

    check_target_program_presence(remaining_accounts, &target_program)?;

    let governance_config_bump = governance_config.load()?.bump;
    execute_proposal_cpi(
        &execute_proposal_data,
        remaining_accounts,
        governance_config,
        governance_config_bump,
    )?;

    let proposal_hash = ExecutableProposal::calculate_hash(
        &target_program,
        &execute_proposal_data.call_data,
        &execute_proposal_data.native_value,
    );

    emit_cpi!(OperatorProposalExecuted {
        hash: proposal_hash,
        target_address: execute_proposal_data.target_address.to_vec(),
        call_data: execute_proposal_data.call_data.call_data,
        native_value: execute_proposal_data.native_value.to_vec(),
    });

    Ok(())
}
