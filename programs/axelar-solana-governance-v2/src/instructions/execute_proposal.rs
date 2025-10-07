use crate::{
    ExecutableProposal, ExecuteProposalData, GovernanceConfig, GovernanceError, ProposalExecuted,
};
use anchor_lang::prelude::*;
use axelar_solana_governance::seed_prefixes;
use solana_program::instruction::Instruction;

#[derive(Accounts)]
#[event_cpi]
#[instruction(execute_proposal_data: ExecuteProposalData)]
pub struct ExecuteProposal<'info> {
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
}

pub fn execute_proposal_handler(
    ctx: Context<ExecuteProposal>,
    execute_proposal_data: ExecuteProposalData,
) -> Result<()> {
    let proposal = &ctx.accounts.proposal_pda;
    let target_program = Pubkey::new_from_array(execute_proposal_data.target_address);

    let clock = Clock::get()?;
    require!(
        clock.unix_timestamp as u64 >= proposal.eta,
        GovernanceError::ProposalNotReady
    );

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

    emit_cpi!(ProposalExecuted {
        hash: proposal_hash,
        target_address: execute_proposal_data.target_address.to_vec(),
        call_data: execute_proposal_data.call_data.call_data,
        native_value: execute_proposal_data.native_value.to_vec(),
        eta: proposal.eta,
    });

    Ok(())
}

pub fn execute_proposal_cpi(
    execute_proposal_data: &ExecuteProposalData,
    remaining_accounts: &[AccountInfo<'_>],
    governance_config: AccountLoader<'_, GovernanceConfig>,
    governance_config_bump: u8,
) -> Result<()> {
    let native_value_u64 = checked_from_u256_le_bytes_to_u64(&execute_proposal_data.native_value)?;
    if native_value_u64 > 0 {
        manual_lamport_transfer(
            execute_proposal_data.clone(),
            remaining_accounts,
            native_value_u64,
            &governance_config,
        )?;
    }

    let account_metas = get_account_metadata(execute_proposal_data);

    // Execute the target program instruction
    solana_program::program::invoke_signed(
        &Instruction {
            program_id: Pubkey::from(execute_proposal_data.target_address),
            accounts: account_metas,
            data: execute_proposal_data.call_data.call_data.clone(),
        },
        remaining_accounts,
        &[&[seed_prefixes::GOVERNANCE_CONFIG, &[governance_config_bump]]],
    )?;

    Ok(())
}

pub fn check_target_program_presence(
    remaining_accounts: &[AccountInfo<'_>],
    target_program: &Pubkey,
) -> Result<()> {
    let program_present = remaining_accounts
        .iter()
        .any(|acc| acc.key() == *target_program);
    require!(program_present, GovernanceError::InvalidTargetProgram);

    Ok(())
}

fn checked_from_u256_le_bytes_to_u64(le_u256: &[u8; 32]) -> Result<u64> {
    if le_u256[8..32].iter().any(|&byte| byte != 0) {
        return Err(GovernanceError::InvalidNativeValue.into());
    }

    let mut u64data: [u8; 8] = [0_u8; 8];
    u64data.copy_from_slice(&le_u256[0..8]);

    Ok(u64::from_le_bytes(u64data))
}

fn get_account_metadata(execute_proposal_data: &ExecuteProposalData) -> Vec<AccountMeta> {
    execute_proposal_data
        .call_data
        .solana_accounts
        .iter()
        .map(|metadata| {
            if metadata.is_writable && metadata.is_signer {
                AccountMeta::new(Pubkey::new_from_array(metadata.pubkey), true)
            } else if metadata.is_writable {
                AccountMeta::new(Pubkey::new_from_array(metadata.pubkey), false)
            } else if metadata.is_signer {
                AccountMeta::new_readonly(Pubkey::new_from_array(metadata.pubkey), true)
            } else {
                AccountMeta::new_readonly(Pubkey::new_from_array(metadata.pubkey), false)
            }
        })
        .collect()
}

fn manual_lamport_transfer(
    execute_proposal_data: ExecuteProposalData,
    remaining_accounts: &[AccountInfo<'_>],
    native_value_u64: u64,
    governance_config: &AccountLoader<'_, GovernanceConfig>,
) -> Result<()> {
    let target_native_value_account = execute_proposal_data
        .call_data
        .solana_native_value_receiver_account
        .as_ref()
        .ok_or(GovernanceError::MissingNativeValueReceiver)?;

    let mut target_account_info: Option<&AccountInfo> = None;
    for account in remaining_accounts {
        if account.key.to_bytes() == target_native_value_account.pubkey.as_slice() {
            target_account_info = Some(account);
            break;
        }
    }

    let target_account_info = target_account_info.ok_or(GovernanceError::TargetAccountNotFound)?;

    // Manual lamport transfer - extract AccountInfo references first
    let governance_account = governance_config.to_account_info();
    let target_account = target_account_info;

    // Now do the transfer
    {
        let mut governance_lamports = governance_account.try_borrow_mut_lamports()?;
        let mut target_lamports = target_account.try_borrow_mut_lamports()?;

        if **governance_lamports < native_value_u64 {
            return Err(GovernanceError::InsufficientFunds.into());
        }

        **governance_lamports = governance_lamports
            .checked_sub(native_value_u64)
            .ok_or(GovernanceError::InsufficientFunds)?;
        **target_lamports = target_lamports
            .checked_add(native_value_u64)
            .ok_or(GovernanceError::ArithmeticOverflow)?;
    }
    Ok(())
}

pub fn check_governance_config_presence(
    governance_config_key: &Pubkey,
    remaining_accounts: &[AccountInfo],
    solana_accounts: &[crate::SolanaAccountMetadata],
) -> Result<()> {
    let governance_config_in_remaining = remaining_accounts
        .iter()
        .any(|account| account.key == governance_config_key);

    if !governance_config_in_remaining {
        return Err(GovernanceError::GovernanceConfigMissing.into());
    }

    let governance_config_in_solana_accounts = solana_accounts
        .iter()
        .any(|metadata| Pubkey::new_from_array(metadata.pubkey) == *governance_config_key);

    if !governance_config_in_solana_accounts {
        return Err(GovernanceError::GovernanceConfigMissing.into());
    }

    Ok(())
}
