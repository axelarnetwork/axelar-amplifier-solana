use anchor_lang::prelude::*;
use solana_axelar_its::program::SolanaAxelarIts;

use crate::Counter;

#[derive(Accounts)]
#[instruction(token_id: [u8; 32])]
pub struct SendInterchainTransfer<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    // The counter account
    #[account(seeds = [Counter::SEED_PREFIX], bump = counter.bump)]
    pub counter: Account<'info, Counter>,

    //
    // Gateway
    //
    pub gateway_root_pda: UncheckedAccount<'info>,
    pub gateway_event_authority: UncheckedAccount<'info>,
    pub gateway_program: UncheckedAccount<'info>,
    pub signing_pda: UncheckedAccount<'info>,

    //
    // Gas Service
    //
    /// The GMP gas treasury account
    #[account(mut)]
    pub gas_treasury: UncheckedAccount<'info>,
    pub gas_service: UncheckedAccount<'info>,
    /// CHECK: checked by the gas service program
    pub gas_event_authority: UncheckedAccount<'info>,

    //
    // ITS
    //
    pub its_root_pda: UncheckedAccount<'info>,
    pub its_program: Program<'info, SolanaAxelarIts>,
    pub its_event_authority: UncheckedAccount<'info>,

    #[account(mut)]
    pub token_manager_pda: UncheckedAccount<'info>,

    //
    // Token Info
    //
    pub token_program: UncheckedAccount<'info>,

    #[account(mut)]
    pub token_mint: UncheckedAccount<'info>,

    #[account(mut)]
    pub counter_pda_ata: UncheckedAccount<'info>,

    pub token_manager_ata: UncheckedAccount<'info>,

    //
    // Misc
    //
    pub system_program: UncheckedAccount<'info>,
}

pub fn send_interchain_transfer_handler(
    ctx: Context<SendInterchainTransfer>,
    token_id: [u8; 32],
    destination_chain: String,
    destination_address: Vec<u8>,
    amount: u64,
    gas_value: u64,
) -> Result<()> {
    msg!(
        "Sending interchain transfer of {} tokens to chain {}",
        amount,
        destination_chain
    );

    let cpi_accounts = solana_axelar_its::cpi::accounts::InterchainTransfer {
        payer: ctx.accounts.payer.to_account_info(),
        authority: ctx.accounts.counter.to_account_info(),
        gateway_root_pda: ctx.accounts.gateway_root_pda.to_account_info(),
        gateway_event_authority: ctx.accounts.gateway_event_authority.to_account_info(),
        gateway_program: ctx.accounts.gateway_program.to_account_info(),
        signing_pda: ctx.accounts.signing_pda.to_account_info(),
        gas_treasury: ctx.accounts.gas_treasury.to_account_info(),
        gas_service: ctx.accounts.gas_service.to_account_info(),
        gas_event_authority: ctx.accounts.gas_event_authority.to_account_info(),
        its_root_pda: ctx.accounts.its_root_pda.to_account_info(),
        program: ctx.accounts.its_program.to_account_info(),
        event_authority: ctx.accounts.its_event_authority.to_account_info(),
        token_manager_pda: ctx.accounts.token_manager_pda.to_account_info(),
        token_program: ctx.accounts.token_program.to_account_info(),
        token_mint: ctx.accounts.token_mint.to_account_info(),
        authority_token_account: ctx.accounts.counter_pda_ata.to_account_info(),
        token_manager_ata: ctx.accounts.token_manager_ata.to_account_info(),
        system_program: ctx.accounts.system_program.to_account_info(),
    };

    let (_transfer_signing_pda, transfer_signing_pda_bump) = Pubkey::find_program_address(
        &[solana_axelar_its::seed_prefixes::INTERCHAIN_TRANSFER_SEED],
        &crate::ID,
    );
    let signer_seeds = &[
        solana_axelar_its::seed_prefixes::INTERCHAIN_TRANSFER_SEED,
        &[transfer_signing_pda_bump],
    ];
    let signer_seeds = &[&signer_seeds[..]];

    let cpi_ctx = CpiContext::new_with_signer(
        ctx.accounts.its_program.to_account_info(),
        cpi_accounts,
        signer_seeds,
    );

    solana_axelar_its::cpi::interchain_transfer(
        cpi_ctx,
        token_id,
        destination_chain,
        destination_address,
        amount,
        gas_value,
        Some(crate::ID),
        Some(transfer_signing_pda_bump),
        None,
    )?;

    Ok(())
}
