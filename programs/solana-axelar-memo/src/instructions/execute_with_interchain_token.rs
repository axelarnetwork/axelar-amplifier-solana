use anchor_lang::prelude::*;
use anchor_spl::token_2022::spl_token_2022;
use solana_axelar_its::{executable::*, executable_with_interchain_token_accounts};

use crate::{log_memo, Counter};

executable_with_interchain_token_accounts!(ExecuteWithInterchainToken);

#[derive(Accounts)]
pub struct ExecuteWithInterchainToken<'info> {
    pub its_executable: AxelarExecuteWithInterchainTokenAccounts<'info>,

    // The counter account
    #[account(mut, seeds = [Counter::SEED_PREFIX], bump = counter.bump)]
    pub counter: Account<'info, Counter>,
}

pub fn execute_with_interchain_token_handler<'info>(
    ctx: Context<'_, '_, '_, 'info, ExecuteWithInterchainToken<'info>>,
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
    log_memo(memo);

    // Increase counter
    ctx.accounts.counter.counter += 1;

    // If extra remaining accounts are provided, attempt to transfer the received
    // tokens to a destination ATA. This demonstrates that the destination program
    // can spend received ITS tokens by signing as the token authority PDA.
    //
    // remaining_accounts[0] = destination ATA to transfer tokens to
    if let Some(destination_ata) = ctx.remaining_accounts.first() {
        msg!("Attempting to transfer received tokens to destination ATA");
        let authority = &ctx.accounts.its_executable.destination_token_authority;

        let decimals = ctx.accounts.its_executable.token_mint.decimals;

        let transfer_ix = spl_token_2022::instruction::transfer_checked(
            &ctx.accounts.its_executable.token_program.key(),
            &ctx.accounts.its_executable.destination_program_ata.key(),
            &ctx.accounts.its_executable.token_mint.key(),
            &destination_ata.key(),
            &authority.key(),
            &[],
            amount,
            decimals,
        )?;

        // Sign with the token authority PDA seeds
        let bump = ctx.bumps.its_executable.destination_token_authority;
        let signer_seeds: &[&[&[u8]]] = &[&[
            solana_axelar_its::seed_prefixes::ITS_TOKEN_AUTHORITY_SEED,
            &[bump],
        ]];

        anchor_lang::solana_program::program::invoke_signed(
            &transfer_ix,
            &[
                ctx.accounts
                    .its_executable
                    .destination_program_ata
                    .to_account_info(),
                ctx.accounts.its_executable.token_mint.to_account_info(),
                destination_ata.clone(),
                authority.to_account_info(),
            ],
            signer_seeds,
        )?;

        msg!("Successfully transferred {} tokens", amount);
    }

    Ok(())
}
