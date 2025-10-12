use crate::GovernanceError;
use anchor_lang::prelude::*;

/// Transfers lamports from source to target account with proper error handling
///
/// # Arguments
/// * `source_account` - The account to transfer lamports from
/// * `target_account` - The account to transfer lamports to
/// * `amount` - The amount of lamports to transfer
/// * `check_sufficient_funds` - Whether to check if source has enough funds before transfer
///
/// # Errors
/// Returns `GovernanceError::InsufficientFunds` if source doesn't have enough lamports
/// Returns `GovernanceError::ArithmeticOverflow` if any arithmetic operation would overflow
pub fn transfer_lamports(
    source_account: &AccountInfo,
    target_account: &AccountInfo,
    amount: u64,
) -> Result<()> {
    let mut source_lamports = source_account.try_borrow_mut_lamports()?;
    let mut target_lamports = target_account.try_borrow_mut_lamports()?;

    if **source_lamports < amount {
        return Err(GovernanceError::InsufficientFunds.into());
    }

    **source_lamports = source_lamports
        .checked_sub(amount)
        .ok_or(GovernanceError::InsufficientFunds)?;

    **target_lamports = target_lamports
        .checked_add(amount)
        .ok_or(GovernanceError::ArithmeticOverflow)?;

    Ok(())
}

use alloy_sol_types::SolType;
use anchor_lang::AnchorDeserialize;
/// A module to convert the payload data of a governance GMP command.
use governance_gmp::alloy_primitives::Bytes;
use governance_gmp::GovernanceCommandPayload;
use solana_program::msg;
use solana_program::program_error::ProgramError;
use solana_program::pubkey::Pubkey;

use crate::ExecuteProposalCallData;

/// Decodes the payload of a governance GMP command.
///
/// # Errors
///
/// A `ProgramError` is returned if the payload cannot be deserialized.
pub fn decode_payload(
    raw_payload: &[u8],
) -> std::result::Result<GovernanceCommandPayload, ProgramError> {
    GovernanceCommandPayload::abi_decode(raw_payload, true).map_err(|err| {
        msg!("Cannot abi decode GovernanceCommandPayload: {}", err);
        ProgramError::InvalidArgument
    })
}

/// Decodes the target address from the payload.
///
/// # Errors
///
/// A `ProgramError` is returned if the target address cannot be deserialized.
pub fn decode_payload_target(
    payload_target_addr: &Bytes,
) -> std::result::Result<Pubkey, ProgramError> {
    let target: [u8; 32] = payload_target_addr.to_vec().try_into().map_err(|_err| {
        msg!("Cannot cast incoming target address for governance gmp command");
        ProgramError::InvalidArgument
    })?;
    Ok(Pubkey::from(target))
}

pub fn decode_payload_call_data(
    call_data: &Bytes,
) -> std::result::Result<ExecuteProposalCallData, ProgramError> {
    ExecuteProposalCallData::try_from_slice(call_data).map_err(|_| ProgramError::InvalidArgument)
}
