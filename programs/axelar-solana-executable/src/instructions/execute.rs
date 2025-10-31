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

pub fn execute_handler(
    ctx: Context<Execute>,
    payload: Payload,
    message: Message,
) -> Result<()> {
    let mut payload_bytes = Vec::new();
    payload.serialize(&mut payload_bytes).unwrap();
    validate_message_raw(&ctx.accounts.axelar_executable(), message, payload_bytes.as_slice())?;

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
