use anchor_lang::prelude::*;
use axelar_solana_gateway_v2::{executable::*, executable_accounts};
use crate::Payload;

executable_accounts!(Execute);

use crate::Counter;

#[derive(Accounts)]
#[instruction(payload: Payload)]
pub struct Execute<'info> {
    // GMP Accounts
    pub executable: AxelarExecuteAccounts<'info>,

    // The counter account
    #[account(mut, seeds = [Counter::SEED_PREFIX, &payload.storage_id.to_le_bytes()], bump)]
    pub counter: Account<'info, Counter>,
}

pub fn execute_handler(
    ctx: Context<Execute>,
    message: Message,
    payload: Payload,
    encoding_scheme: axelar_solana_gateway_v2::executable::ExecutablePayloadEncodingScheme,
) -> Result<()> {
    let mut payload_bytes = Vec::new();
    payload.serialize(&mut payload_bytes).unwrap();
    validate_message(ctx.accounts, message, &payload_bytes, encoding_scheme)?;

    msg!("Payload size: {}", payload_bytes.len());

    // Log memo
    log_memo(&payload.memo);

    // Increase counter
    ctx.accounts.counter.counter += 1;
    

    Ok(())
}

#[inline]
fn log_memo(memo: &String) {
    // If memo is longer than 10 characters, log just the first character.
    let char_count = memo.chars().count();
    if char_count > 10 {
        msg!(
            "Memo (len {}): {:?} x {} (too big to log)",
            memo.len(),
            memo.chars().next().unwrap(),
            char_count
        );
    } else {
        msg!("Memo (len {}): {:?}", memo.len(), memo);
    }
}
