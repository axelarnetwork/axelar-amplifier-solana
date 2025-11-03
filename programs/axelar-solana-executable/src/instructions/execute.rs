use anchor_lang::prelude::*;
use axelar_solana_gateway_v2::{executable::*, executable_accounts};
use crate::Payload;

executable_accounts!(Execute);

use crate::Counter;

#[derive(Accounts)]
#[instruction(payload: Payload)]
/// The execute entrypoint.
pub struct Execute<'info> {
    // GMP Accounts
    pub executable: AxelarExecuteAccounts<'info>,

    #[account(mut)]
    pub payer: Signer<'info>,

    // The counter account
    #[account(
        init_if_needed,
        space = Counter::DISCRIMINATOR.len() + Counter::INIT_SPACE,
        payer = payer,
        seeds = [Counter::SEED_PREFIX, &payload.storage_id.to_le_bytes()], 
        bump
    )]
    pub counter: Account<'info, Counter>,

    pub system_program: Program<'info, System>,
}

/// This function keeps track of how many times a message has been received for a given `payload.storage_id`, and logs the `payload.memo`.
pub fn execute_handler(
    ctx: Context<Execute>,
    payload: Payload,
    message: Message,
) -> Result<()> {
    // serialize the payload into a `Vec<u8>`. Haven't found a single function that does this which is surprising, must be missing something.
    let mut payload_bytes = Vec::new();
    payload.serialize(&mut payload_bytes).unwrap();

    // Validate the message with the gateway.
    validate_message_raw(&ctx.accounts.axelar_executable(), message, payload_bytes.as_slice())?;

    // Log the payload size.
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
