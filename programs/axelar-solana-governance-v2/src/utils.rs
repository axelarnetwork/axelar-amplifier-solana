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
