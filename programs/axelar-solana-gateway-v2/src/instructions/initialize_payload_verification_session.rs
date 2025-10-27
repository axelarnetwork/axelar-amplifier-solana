use crate::{
    verification_session::SignatureVerificationSessionData, GatewayConfig, GatewayError,
    VerifierSetTracker,
};
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(merkle_root: [u8; 32])]
pub struct InitializePayloadVerificationSession<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        seeds = [GatewayConfig::SEED_PREFIX],
        bump = gateway_root_pda.load()?.bump
    )]
    pub gateway_root_pda: AccountLoader<'info, GatewayConfig>,

    /// CHECK: PDA validation is performed manually in the handler after loading verifier_set_hash
    #[account(
        init,
        payer = payer,
        space = SignatureVerificationSessionData::DISCRIMINATOR.len() + std::mem::size_of::<SignatureVerificationSessionData>(),
    )]
    pub verification_session_account: AccountLoader<'info, SignatureVerificationSessionData>,

    /// CHECK: PDA validation is performed manually in the handler
    pub verifier_set_tracker_pda: AccountLoader<'info, VerifierSetTracker>,

    pub system_program: Program<'info, System>,
}

pub fn initialize_payload_verification_session_handler(
    ctx: Context<InitializePayloadVerificationSession>,
    merkle_root: [u8; 32],
) -> Result<()> {
    // Load verifier_set_hash from the provided account
    let verifier_set_tracker = ctx.accounts.verifier_set_tracker_pda.load()?;
    let verifier_set_hash = verifier_set_tracker.verifier_set_hash;

    // Validate that the verifier set isn't expired
    ctx.accounts
        .gateway_root_pda
        .load()?
        .assert_valid_epoch(verifier_set_tracker.epoch)
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

    // Manually validate verification_session_account PDA
    let (expected_verification_session, bump) = Pubkey::find_program_address(
        &[
            SignatureVerificationSessionData::SEED_PREFIX,
            merkle_root.as_ref(),
            verifier_set_hash.as_ref(),
        ],
        &crate::ID,
    );
    require_keys_eq!(
        ctx.accounts.verification_session_account.key(),
        expected_verification_session,
        GatewayError::InvalidVerifierSetTrackerProvided // TODO: add proper error
    );

    let verification_session_account =
        &mut ctx.accounts.verification_session_account.load_init()?;

    verification_session_account.bump = bump;
    verification_session_account
        .signature_verification
        .signing_verifier_set_hash = verifier_set_hash;

    Ok(())
}
