use anchor_lang::prelude::*;
use solana_axelar_its::{executable::*, executable_with_interchain_token_accounts};

use crate::{log_memo, Counter};

executable_with_interchain_token_accounts!(ExecuteWithInterchainToken);

#[derive(Accounts)]
pub struct ExecuteWithInterchainToken<'info> {
    pub its_executable: AxelarExecuteWithInterchainToken<'info>,

    // The counter account
    #[account(mut, seeds = [Counter::SEED_PREFIX], bump = counter.bump)]
    pub counter: Account<'info, Counter>,
}

pub fn execute_with_interchain_token_handler(
    ctx: Context<ExecuteWithInterchainToken>,
    execute_payload: ExecuteWithInterchainTokenPayload,
) -> Result<()> {
    validate_its_executable(&ctx.accounts.its_executable)?;

    msg!("execute_with_interchain_token_handler called");

    let amount = execute_payload.amount;
    let token = execute_payload.token_id;
    msg!("Received {amount} interchain token id: {token}");

    let payload = execute_payload.data;

    msg!("Payload size: {}", payload.len());
    let memo = std::str::from_utf8(&payload).map_err(|err| {
        msg!("Invalid UTF-8, from byte {}", err.valid_up_to());
        ProgramError::InvalidInstructionData
    })?;

    // Log memo
    log_memo(memo);

    // // Increase counter
    ctx.accounts.counter.counter += 1;

    Ok(())
}
