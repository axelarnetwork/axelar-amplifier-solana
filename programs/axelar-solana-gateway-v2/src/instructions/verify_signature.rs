#![allow(clippy::empty_structs_with_brackets)]
use crate::seed_prefixes::{GATEWAY_SEED, SIGNATURE_VERIFICATION_SEED, VERIFIER_SET_TRACKER_SEED};
use crate::{
    verification_session::SigningVerifierSetInfo, GatewayConfig, GatewayError,
    SignatureVerificationSessionData, VerifierSetTracker,
};
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(merkle_root: [u8; 32], verifier_info: SigningVerifierSetInfo)]
pub struct VerifySignature<'info> {
    #[account(
        seeds = [GATEWAY_SEED],
        bump = gateway_root_pda.load()?.bump,
        // Check: Verifier domain separator matches the gateway's domain separator
        constraint = gateway_root_pda.load()?.domain_separator == verifier_info.leaf.domain_separator
        	@ GatewayError::InvalidDomainSeparator,
        // Check: Verifier set isn't expired
        constraint = gateway_root_pda.load()?.assert_valid_epoch(verifier_set_tracker_pda.epoch).is_ok()
        	@ GatewayError::VerifierSetTooOld,
    )]
    pub gateway_root_pda: AccountLoader<'info, GatewayConfig>,

    #[account(
        mut,
        seeds = [SIGNATURE_VERIFICATION_SEED, merkle_root.as_ref()],
        bump = verification_session_account.load()?.bump
    )]
    pub verification_session_account: AccountLoader<'info, SignatureVerificationSessionData>,

    #[account(
		// The verifier set tracker PDA is derived from the verifier set hash
		seeds = [VERIFIER_SET_TRACKER_SEED, verifier_set_tracker_pda.verifier_set_hash.as_slice()],
		bump,
	)]
    pub verifier_set_tracker_pda: Account<'info, VerifierSetTracker>,
}

pub fn verify_signature_handler(
    ctx: Context<VerifySignature>,
    payload_merkle_root: [u8; 32],
    verifier_info: SigningVerifierSetInfo,
) -> Result<()> {
    // Verify signature
    ctx.accounts
        .verification_session_account
        .load_mut()?
        .process_signature(
            payload_merkle_root,
            &ctx.accounts.verifier_set_tracker_pda.verifier_set_hash,
            verifier_info,
        )?;

    Ok(())
}
