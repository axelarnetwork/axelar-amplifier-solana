use anchor_lang::prelude::*;
use solana_axelar_gateway::{
    cpi::accounts::CallContract, program::SolanaAxelarGateway, CallContractSigner,
};

#[derive(Accounts)]
pub struct SendMemo<'info> {
    /// Reference to our program
    pub memo_program: Program<'info, crate::program::Memo>,

    /// CHECK:
    /// Our standardized PDA for calling the gateway
    #[account(
        seeds = [CallContractSigner::SEED_PREFIX],
        bump,
    )]
    pub signing_pda: AccountInfo<'info>,

    /// The gateway configuration PDA
    /// CHECK: checked by the gateway program
    pub gateway_root_pda: UncheckedAccount<'info>,

    /// Event authority - derived from gateway program
    /// CHECK: checked by the gateway program
    pub gateway_event_authority: UncheckedAccount<'info>,

    /// Reference to the axelar gateway program
    pub gateway_program: Program<'info, SolanaAxelarGateway>,
}

pub fn send_memo_handler(
    ctx: Context<SendMemo>,
    destination_chain: String,
    destination_address: String,
    memo: String,
) -> Result<()> {
    msg!(
        "Sending memo: '{}' to chain: {} at contract: {}",
        memo,
        destination_chain,
        destination_address
    );

    let payload = memo.as_bytes().to_vec();
    let bump = ctx.bumps.signing_pda;

    let signer_seeds = &[CallContractSigner::SEED_PREFIX, &[bump]];
    let signer_seeds = &[&signer_seeds[..]];

    let cpi_accounts = CallContract {
        caller: ctx.accounts.memo_program.to_account_info(),
        signing_pda: Some(ctx.accounts.signing_pda.to_account_info()),
        gateway_root_pda: ctx.accounts.gateway_root_pda.to_account_info(),
        // For event_cpi
        event_authority: ctx.accounts.gateway_event_authority.to_account_info(),
        program: ctx.accounts.gateway_program.to_account_info(),
    };

    let cpi_ctx = CpiContext::new_with_signer(
        ctx.accounts.gateway_program.key(),
        cpi_accounts,
        signer_seeds,
    );

    solana_axelar_gateway::cpi::call_contract(
        cpi_ctx,
        destination_chain,
        destination_address,
        payload,
        bump,
    )?;

    msg!("Memo sent successfully!");
    Ok(())
}
