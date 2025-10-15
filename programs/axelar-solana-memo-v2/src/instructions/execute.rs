use anchor_lang::prelude::*;
use axelar_solana_gateway_v2::executable::*;

use crate::{accounts, Counter};

#[derive(Accounts)]
pub struct Execute<'info> {
    // GMP Accounts
    pub executable: AxelarExecuteAccounts<'info>,

    // The counter account
    #[account(mut, seeds = [Counter::SEED_PREFIX], bump)]
    pub counter: Account<'info, Counter>,
}

pub fn execute_handler(ctx: Context<Execute>, message: Message, payload: Vec<u8>) -> Result<()> {
    msg!("Executing payload of: {} bytes", payload.len());

    validate_message(&ctx.accounts.executable, message, &payload)?;

    // example action
    ctx.accounts.counter.counter += 1;

    Ok(())
}
