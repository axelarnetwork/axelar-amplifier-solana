use anchor_lang::prelude::*;
use axelar_solana_gateway::seed_prefixes::{CALL_CONTRACT_SIGNING_SEED, GATEWAY_SEED};
use axelar_solana_gateway_v2::{
    cpi::accounts::CallContract, program::AxelarSolanaGatewayV2, GatewayConfig,
};

#[derive(Accounts)]
pub struct SendMemo<'info> {
    /// Our standardized PDA for calling the gateway
    #[account(
        seeds = [CALL_CONTRACT_SIGNING_SEED],
        bump,
        seeds::program = crate::ID
    )]
    pub gateway_caller_pda: Signer<'info>,

    /// Reference to our program
    /// CHECK: this is enforced to be our programId
    #[account(address = crate::ID)]
    pub memo_program: UncheckedAccount<'info>,
    /// Reference to the axelar gateway program
    pub axelar_gateway_program: Program<'info, AxelarSolanaGatewayV2>,
    /// The gateway configuration PDA being initialized
    #[account(
            seeds = [GATEWAY_SEED],
            bump = gateway_root_pda.bump,
            seeds::program = axelar_gateway_program.key()
        )]
    pub gateway_root_pda: Account<'info, GatewayConfig>,
    /// Event authority - derived from gateway program
    #[account(
            seeds = [b"__event_authority"],
            bump,
            seeds::program = axelar_gateway_program.key()
        )]
    pub event_authority: SystemAccount<'info>,
    pub system_program: Program<'info, System>,
}

pub fn send_memo_handler(
    ctx: Context<SendMemo>,
    destination_chain: String,
    destination_contract_address: String,
    memo: String,
) -> Result<()> {
    msg!(
        "Sending memo: '{}' to chain: {} at contract: {}",
        memo,
        destination_chain,
        destination_contract_address
    );

    let payload = memo.as_bytes().to_vec();
    let bump = ctx.bumps.gateway_caller_pda;
    let signer_seeds: &[&[&[u8]]] = &[&[CALL_CONTRACT_SIGNING_SEED, &[bump]]];

    let cpi_accounts = CallContract {
        calling_program: ctx.accounts.memo_program.to_account_info(),
        signing_pda: ctx.accounts.gateway_caller_pda.to_account_info(),
        gateway_root_pda: ctx.accounts.gateway_root_pda.to_account_info(),
        // For event_cpi
        event_authority: ctx.accounts.event_authority.to_account_info(),
        program: ctx.accounts.axelar_gateway_program.to_account_info(),
    };

    let cpi_ctx = CpiContext::new_with_signer(
        ctx.accounts.axelar_gateway_program.to_account_info(),
        cpi_accounts,
        signer_seeds,
    );

    axelar_solana_gateway_v2::cpi::call_contract(
        cpi_ctx,
        destination_chain,
        destination_contract_address,
        payload,
    )?;

    msg!("Memo sent successfully!");
    Ok(())
}
