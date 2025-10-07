use crate::{ExecutableProposal, GovernanceConfig, GovernanceError, ProposalScheduled};
use anchor_lang::prelude::*;
use axelar_solana_governance::seed_prefixes;

#[derive(Accounts)]
#[event_cpi]
#[instruction(proposal_hash: [u8; 32], eta: u64, native_value: Vec<u8>, target: Vec<u8>, call_data: Vec<u8>)]
pub struct ScheduleTimelockProposal<'info> {
    pub system_program: Program<'info, System>,
    #[account(
            seeds = [axelar_solana_governance::seed_prefixes::GOVERNANCE_CONFIG],
            bump = governance_config.load()?.bump,
        )]
    pub governance_config: AccountLoader<'info, GovernanceConfig>,
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(
            init,
            payer = payer,
            space = ExecutableProposal::DISCRIMINATOR.len() + std::mem::size_of::<ExecutableProposal>(),
            seeds = [axelar_solana_governance::seed_prefixes::PROPOSAL_PDA, &proposal_hash],
            bump
        )]
    pub proposal_pda: AccountLoader<'info, ExecutableProposal>,
}

pub fn schedule_timelock_proposal_instruction_handler(
    ctx: Context<ScheduleTimelockProposal>,
    proposal_hash: [u8; 32],
    eta: u64,
    native_value: Vec<u8>,
    target: Vec<u8>,
    call_data: Vec<u8>,
) -> Result<()> {
    let (_, managed_bump) = Pubkey::find_program_address(
        &[seed_prefixes::OPERATOR_MANAGED_PROPOSAL, &proposal_hash],
        &crate::ID,
    );

    // Enforce min delay
    let eta = at_least_default_eta_delay(
        eta,
        ctx.accounts
            .governance_config
            .load()?
            .minimum_proposal_eta_delay,
    )?;

    let proposal_pda = &mut ctx.accounts.proposal_pda.load_init()?;
    proposal_pda.eta = eta;
    proposal_pda.managed_bump = managed_bump;
    proposal_pda.bump = ctx.bumps.proposal_pda;

    emit_cpi!(ProposalScheduled {
        hash: proposal_hash,
        target_address: target,
        call_data,
        native_value,
        eta,
    });

    Ok(())
}

fn at_least_default_eta_delay(proposal_time: u64, min_eta_delay: u32) -> Result<u64> {
    let clock = Clock::get()?;
    let now = clock.unix_timestamp as u64;

    let minimum_proposal_eta = now.checked_add(min_eta_delay as u64).ok_or_else(|| {
        msg!("Overflow when calculating minimum proposal ETA");
        GovernanceError::ArithmeticOverflow
    })?;

    if proposal_time < minimum_proposal_eta {
        Ok(minimum_proposal_eta)
    } else {
        Ok(proposal_time)
    }
}
