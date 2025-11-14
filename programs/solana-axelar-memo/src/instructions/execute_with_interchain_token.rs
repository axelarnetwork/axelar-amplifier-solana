use anchor_lang::prelude::*;
use solana_axelar_its::executable::*;

use crate::Counter;

#[derive(Accounts)]
pub struct ExecuteWithInterchainToken<'info> {
    // The counter account
    #[account(mut, seeds = [Counter::SEED_PREFIX], bump = counter.bump)]
    pub counter: Account<'info, Counter>,
}

pub fn execute_with_interchain_token_handler(
    _ctx: Context<ExecuteWithInterchainToken>,
    _execute_payload: ExecuteWithInterchainTokenPayload,
) -> Result<()> {
    // validate_message(ctx.accounts, message, &payload, encoding_scheme)?;

    // msg!("Payload size: {}", payload.len());
    // let memo = std::str::from_utf8(&payload).map_err(|err| {
    //     msg!("Invalid UTF-8, from byte {}", err.valid_up_to());
    //     ProgramError::InvalidInstructionData
    // })?;

    // // Log memo
    // log_memo(memo);

    // // Increase counter
    // ctx.accounts.counter.counter += 1;

    Ok(())
}
