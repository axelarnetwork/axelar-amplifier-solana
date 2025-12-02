use anchor_lang::{prelude::*, AnchorDeserialize};
use solana_axelar_its::{executable::*, executable_with_interchain_token_accounts};

use crate::{log_memo, Counter, Payload};

executable_with_interchain_token_accounts!(ExecuteWithInterchainToken);

#[derive(Accounts)]
#[instruction(execute_payload: AxelarExecuteWithInterchainTokenPayload)]
pub struct ExecuteWithInterchainToken<'info> {
    pub its_executable: AxelarExecuteWithInterchainTokenAccounts<'info>,

    #[account(mut)]
    pub payer: Signer<'info>,

    // The counter account
    #[account(
        init_if_needed,
        space = Counter::DISCRIMINATOR.len() + Counter::INIT_SPACE,
        payer = payer,
        seeds = [
            Counter::SEED_PREFIX,
            &Payload::deserialize(&mut execute_payload.data.as_slice()).unwrap().storage_id.to_ne_bytes(),
        ],
        bump,
    )]
    pub counter: Account<'info, Counter>,

    pub system_program: Program<'info, System>,
}

pub fn execute_with_interchain_token_handler(
    ctx: Context<ExecuteWithInterchainToken>,
    execute_payload: AxelarExecuteWithInterchainTokenPayload,
) -> Result<()> {
    msg!("execute_with_interchain_token_handler called");

    let amount = execute_payload.amount;
    let token = execute_payload.token_id;
    msg!("Received {} interchain token id: {:?}", amount, token);
    msg!("Token mint: {}", execute_payload.token_mint);

    let memo_data = execute_payload.data;

    msg!("Payload size: {}", memo_data.len());
    let memo = std::str::from_utf8(&memo_data).map_err(|err| {
        msg!("Invalid UTF-8, from byte {}", err.valid_up_to());
        ProgramError::InvalidInstructionData
    })?;

    // Log memo
    log_memo(&String::from(memo));

    // // Increase counter
    ctx.accounts.counter.counter += 1;

    Ok(())
}
