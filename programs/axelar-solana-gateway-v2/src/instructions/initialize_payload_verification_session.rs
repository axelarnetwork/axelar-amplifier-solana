use crate::seed_prefixes::{GATEWAY_SEED, SIGNATURE_VERIFICATION_SEED, VERIFIER_SET_TRACKER_SEED};
use crate::{verification_session::SignatureVerificationSessionData, GatewayConfig, GatewayError, VerifierSetTracker};
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(merkle_root: [u8; 32], signing_verifier_set_hash: [u8; 32])]
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
        seeds = [SIGNATURE_VERIFICATION_SEED, merkle_root.as_ref(), signing_verifier_set_hash.as_ref()],
        bump
    )]
    pub verification_session_account: AccountLoader<'info, SignatureVerificationSessionData>,

    #[account(
        seeds = [VERIFIER_SET_TRACKER_SEED, signing_verifier_set_hash.as_ref()],
        bump,
        // Validate that the provided hash matches the tracker's hash
        constraint = verifier_set_tracker_pda.load()?.verifier_set_hash == signing_verifier_set_hash
            @ GatewayError::InvalidVerifierSetTrackerProvided,
        // Validate that the verifier set isn't expired
        constraint = gateway_root_pda.load()?.assert_valid_epoch(verifier_set_tracker_pda.load()?.epoch).is_ok()
            @ GatewayError::VerifierSetTooOld,
    )]
    pub verifier_set_tracker_pda: AccountLoader<'info, VerifierSetTracker>,

    pub system_program: Program<'info, System>,
}

pub fn initialize_payload_verification_session_handler(
    ctx: Context<InitializePayloadVerificationSession>,
    _merkle_root: [u8; 32],
    _signing_verifier_set_hash: [u8; 32],
) -> Result<()> {
    let verification_session_account =
        &mut ctx.accounts.verification_session_account.load_init()?;

    verification_session_account.bump = ctx.bumps.verification_session_account;

    Ok(())
}
