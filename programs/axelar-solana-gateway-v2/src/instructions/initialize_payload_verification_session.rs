use crate::seed_prefixes::{GATEWAY_SEED, SIGNATURE_VERIFICATION_SEED};
use crate::{verification_session::SignatureVerificationSessionData, GatewayConfig};
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(merkle_root: [u8; 32])]
pub struct InitializePayloadVerificationSession<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        seeds = [GATEWAY_SEED],
        bump = gateway_root_pda.load()?.bump
    )]
    pub gateway_root_pda: AccountLoader<'info, GatewayConfig>,

    #[account(
        init,
        payer = payer,
        space = SignatureVerificationSessionData::DISCRIMINATOR.len() + std::mem::size_of::<SignatureVerificationSessionData>(),
        seeds = [SIGNATURE_VERIFICATION_SEED, merkle_root.as_ref()],
        bump
    )]
    pub verification_session_account: AccountLoader<'info, SignatureVerificationSessionData>,

    pub system_program: Program<'info, System>,
}

pub fn initialize_payload_verification_session_handler(
    ctx: Context<InitializePayloadVerificationSession>,
    _merkle_root: [u8; 32],
) -> Result<()> {
    let verification_session_account =
        &mut ctx.accounts.verification_session_account.load_init()?;

    verification_session_account.bump = ctx.bumps.verification_session_account;

    Ok(())
}
