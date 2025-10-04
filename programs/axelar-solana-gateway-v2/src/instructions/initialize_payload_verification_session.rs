use crate::{verification_session::SignatureVerificationSessionData, GatewayConfig};
use anchor_lang::prelude::*;
use axelar_solana_gateway::seed_prefixes::{GATEWAY_SEED, SIGNATURE_VERIFICATION_SEED};

#[derive(Accounts)]
#[instruction(merkle_root: [u8; 32])]
pub struct InitializePayloadVerificationSession<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(
            seeds = [GATEWAY_SEED],
            bump = gateway_root_pda.bump
        )]
    pub gateway_root_pda: Account<'info, GatewayConfig>,
    #[account(
            init,
            payer = payer,
            space = SignatureVerificationSessionData::DISCRIMINATOR.len() + std::mem::size_of::<SignatureVerificationSessionData>(),
            seeds = [SIGNATURE_VERIFICATION_SEED, merkle_root.as_ref()],
            bump
        )]
    pub verification_session_account: Account<'info, SignatureVerificationSessionData>,
    pub system_program: Program<'info, System>,
}

pub fn initialize_payload_verification_session_handler(
    ctx: Context<InitializePayloadVerificationSession>,
    _merkle_root: [u8; 32],
) -> Result<()> {
    let bump = ctx.bumps.verification_session_account;
    ctx.accounts.verification_session_account.bump = bump;
    Ok(())
}
