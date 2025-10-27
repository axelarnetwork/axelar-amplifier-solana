#![allow(clippy::empty_structs_with_brackets)]
use crate::{
    verification_session::SigningVerifierSetInfo, GatewayConfig, GatewayError,
    SignatureVerificationSessionData, VerifierSetTracker,
};
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(payload_merkle_root: [u8; 32], verifier_info: SigningVerifierSetInfo)]
pub struct VerifySignature<'info> {
    #[account(
        seeds = [GatewayConfig::SEED_PREFIX],
        bump = gateway_root_pda.load()?.bump,
    )]
    pub gateway_root_pda: AccountLoader<'info, GatewayConfig>,

    /// CHECK: PDA validation is performed manually in the handler
    #[account(mut)]
    pub verification_session_account: AccountLoader<'info, SignatureVerificationSessionData>,

    /// CHECK: PDA validation is performed manually in the handler
    pub verifier_set_tracker_pda: AccountLoader<'info, VerifierSetTracker>,
}

pub fn verify_signature_handler(
    ctx: Context<VerifySignature>,
    payload_merkle_root: [u8; 32],
    verifier_info: SigningVerifierSetInfo,
) -> Result<()> {
    let verifier_set_tracker_pda = ctx.accounts.verifier_set_tracker_pda.load()?;
    let verifier_set_hash = verifier_set_tracker_pda.verifier_set_hash;

    // Check: Verifier domain separator matches the gateway's domain separator
    require!(
        ctx.accounts.gateway_root_pda.load()?.domain_separator
            == verifier_info.leaf.domain_separator,
        GatewayError::InvalidDomainSeparator
    );

    // Check: Verifier set isn't expired
    ctx.accounts
        .gateway_root_pda
        .load()?
        .assert_valid_epoch(verifier_set_tracker_pda.epoch)
        .map_err(|_| GatewayError::VerifierSetTooOld)?;

    // Manually validate verifier_set_tracker_pda
    let (expected_verifier_set_tracker, _bump) = Pubkey::find_program_address(
        &[VerifierSetTracker::SEED_PREFIX, verifier_set_hash.as_ref()],
        &crate::ID,
    );
    require_keys_eq!(
        ctx.accounts.verifier_set_tracker_pda.key(),
        expected_verifier_set_tracker,
        GatewayError::InvalidVerifierSetTrackerProvided
    );

    // Manually validate verification_session_account
    let (expected_verification_session, _bump) = Pubkey::find_program_address(
        &[
            SignatureVerificationSessionData::SEED_PREFIX,
            payload_merkle_root.as_ref(),
            verifier_set_hash.as_ref(),
        ],
        &crate::ID,
    );
    require_keys_eq!(
        ctx.accounts.verification_session_account.key(),
        expected_verification_session,
        GatewayError::InvalidVerifierSetTrackerProvided // TODO: add proper error
    );

    // Verify signature
    ctx.accounts
        .verification_session_account
        .load_mut()?
        .process_signature(payload_merkle_root, &verifier_set_hash, verifier_info)?;

    Ok(())
}
