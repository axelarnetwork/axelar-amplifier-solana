use crate::state::Config;
use anchor_lang::solana_program::log::sol_log_data;
use anchor_lang::{prelude::*, system_program};
use axelar_solana_gas_service_events::event_prefixes;

/// Add more native SOL gas to an existing transaction.
///
/// Accounts expected:
/// 1. `[signer, writable]` The account (`sender`) providing the additional lamports.
/// 2. `[writable]` The `config_pda` account that receives the additional lamports.
/// 3. `[]` The `system_program` account.
#[derive(Accounts)]
pub struct AddNativeGas<'info> {
    #[account(mut)]
    pub sender: Signer<'info>,

    #[account(
    	mut,
        seeds = [
            Config::SEED_PREFIX,
        ],
        bump = config_pda.load()?.bump,
    )]
    pub config_pda: AccountLoader<'info, Config>,

    pub system_program: Program<'info, System>,
}

pub fn add_native_gas(
    ctx: Context<AddNativeGas>,
    tx_hash: [u8; 64],
    log_index: u64,
    gas_fee_amount: u64,
    refund_address: Pubkey,
) -> Result<()> {
    if gas_fee_amount == 0 {
        msg!("Gas fee amount cannot be zero");
        return Err(ProgramError::InvalidInstructionData.into());
    }

    let config_pda_account_info = &ctx.accounts.config_pda.to_account_info();

    system_program::transfer(
        CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            system_program::Transfer {
                from: ctx.accounts.sender.to_account_info(),
                to: config_pda_account_info.clone(),
            },
        ),
        gas_fee_amount,
    )?;

    // Emit an event
    sol_log_data(&[
        event_prefixes::NATIVE_GAS_ADDED,
        &config_pda_account_info.key.to_bytes(),
        &tx_hash,
        &log_index.to_le_bytes(),
        &refund_address.to_bytes(),
        &gas_fee_amount.to_le_bytes(),
    ]);

    Ok(())
}
