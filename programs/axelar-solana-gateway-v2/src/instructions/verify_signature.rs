use crate::{
    signature_verification::{SigningVerifierSetInfo, VerificationSessionAccount},
    GatewayConfig, GatewayError, VerifierSetTracker,
};
use anchor_lang::prelude::*;
use axelar_solana_gateway::seed_prefixes::{
    GATEWAY_SEED, SIGNATURE_VERIFICATION_SEED, VERIFIER_SET_TRACKER_SEED,
};

#[derive(Accounts)]
#[instruction(verify_signature_instruction: VerifySignatureInstruction)]
pub struct VerifySignature<'info> {
    #[account(
            seeds = [GATEWAY_SEED],
            bump = gateway_root_pda.bump
        )]
    pub gateway_root_pda: Account<'info, GatewayConfig>,
    #[account(
            mut,
            seeds = [SIGNATURE_VERIFICATION_SEED, verify_signature_instruction.payload_merkle_root.as_ref()],
            bump = verification_session_account.bump
        )]
    pub verification_session_account: Account<'info, VerificationSessionAccount>,
    pub verifier_set_tracker_pda: Account<'info, VerifierSetTracker>,
}

#[derive(Debug, AnchorSerialize, AnchorDeserialize)]
pub struct VerifySignatureInstruction {
    _padding: u8,
    pub payload_merkle_root: [u8; 32],
    pub verifier_info: SigningVerifierSetInfo,
}

impl VerifySignatureInstruction {
    pub fn new(payload_merkle_root: [u8; 32], verifier_info: SigningVerifierSetInfo) -> Self {
        VerifySignatureInstruction {
            _padding: 0u8,
            payload_merkle_root,
            verifier_info,
        }
    }
}

pub fn verify_signature_handler(
    ctx: Context<VerifySignature>,
    verify_signature_instruction: VerifySignatureInstruction,
) -> Result<()> {
    let payload_merkle_root = verify_signature_instruction.payload_merkle_root;
    let verifier_info = verify_signature_instruction.verifier_info;

    let epoch = ctx.accounts.verifier_set_tracker_pda.epoch;
    let current_epoch = ctx.accounts.gateway_root_pda.current_epoch;

    let elapsed = current_epoch
        .checked_sub(epoch)
        .ok_or(GatewayError::EpochCalculationOverflow)?;

    // Check: Verifier set isn't expired
    if elapsed
        >= ctx
            .accounts
            .gateway_root_pda
            .previous_verifier_set_retention
    {
        return err!(GatewayError::VerifierSetTooOld);
    }

    // Check: Verifier domain separator matches the gateway's domain separator
    if verifier_info.leaf.domain_separator != ctx.accounts.gateway_root_pda.domain_separator {
        return err!(GatewayError::InvalidDomainSeparator);
    }

    let expected_verifier_set_hash = &ctx.accounts.verifier_set_tracker_pda.verifier_set_hash;

    // Derive the expected PDA for this verifier set hash
    let (expected_pda, _) = Pubkey::find_program_address(
        &[
            VERIFIER_SET_TRACKER_SEED,
            expected_verifier_set_hash.as_slice(),
        ],
        ctx.program_id,
    );

    // Ensure the provided PDA matches what we expect
    if expected_pda != ctx.accounts.verifier_set_tracker_pda.key() {
        return err!(GatewayError::InvalidVerifierSetTrackerPDA);
    }

    // Verify signature
    ctx.accounts
        .verification_session_account
        .process_signature(
            payload_merkle_root,
            &ctx.accounts.verifier_set_tracker_pda.verifier_set_hash,
            verifier_info,
        )?;

    Ok(())
}
