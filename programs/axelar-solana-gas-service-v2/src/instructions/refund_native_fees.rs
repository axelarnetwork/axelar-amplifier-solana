use crate::state::Config;
use anchor_lang::prelude::*;
use anchor_lang::solana_program::log::sol_log_data;
use axelar_solana_gas_service_events::event_prefixes;

/// Refund previously collected native SOL fees (operator only).
///
/// Accounts expected:
/// 1. `[signer, read-only]` The `operator` account authorized to issue refunds.
/// 2. `[writable]` The `receiver` account that will receive the refunded lamports.
/// 3. `[writable]` The `config_pda` account from which lamports are refunded.
#[derive(Accounts)]
pub struct RefundNativeFees<'info> {
    #[account(address = config_pda.load()?.operator)]
    pub operator: Signer<'info>,

    /// CHECK: Can be any account to receive funds
    #[account(mut)]
    pub receiver: UncheckedAccount<'info>,

    #[account(
    	mut,
        seeds = [
            Config::SEED_PREFIX,
        ],
        bump = config_pda.load()?.bump,
    )]
    pub config_pda: AccountLoader<'info, Config>,
}

pub fn refund_native_fees(
    ctx: Context<RefundNativeFees>,
    tx_hash: [u8; 64],
    log_index: u64,
    fees: u64,
) -> Result<()> {
    // TODO(v2) consider making this a utility function in program-utils
    // similar to transfer_lamports
    if ctx.accounts.config_pda.get_lamports() < fees {
        return Err(ProgramError::InsufficientFunds.into());
    }
    ctx.accounts.config_pda.sub_lamports(fees)?;
    ctx.accounts.receiver.add_lamports(fees)?;

    // Emit an event
    sol_log_data(&[
        event_prefixes::NATIVE_GAS_REFUNDED,
        &tx_hash,
        &ctx.accounts.config_pda.to_account_info().key.to_bytes(),
        &log_index.to_le_bytes(),
        &ctx.accounts.receiver.to_account_info().key.to_bytes(),
        &fees.to_le_bytes(),
    ]);

    Ok(())
}
