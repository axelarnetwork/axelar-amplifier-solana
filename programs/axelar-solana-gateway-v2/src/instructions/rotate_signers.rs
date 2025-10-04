use crate::seed_prefixes::{GATEWAY_SEED, SIGNATURE_VERIFICATION_SEED, VERIFIER_SET_TRACKER_SEED};
use crate::{
    u256::U256, GatewayConfig, GatewayError, SignatureVerificationSessionData,
    VerifierSetRotatedEvent, VerifierSetTracker,
};
use anchor_lang::prelude::*;
use anchor_lang::solana_program;

#[derive(Accounts)]
#[event_cpi]
#[instruction(new_verifier_set_merkle_root: [u8; 32])]
pub struct RotateSigners<'info> {
    #[account(
            mut,
            seeds = [GATEWAY_SEED],
            bump = gateway_root_pda.bump
        )]
    pub gateway_root_pda: Account<'info, GatewayConfig>,
    #[account(
            seeds = [SIGNATURE_VERIFICATION_SEED, construct_payload_hash(new_verifier_set_merkle_root, verification_session_account.signature_verification
            .signing_verifier_set_hash).as_ref()],
            bump = verification_session_account.bump
        )]
    pub verification_session_account: Account<'info, SignatureVerificationSessionData>,
    #[account(
            seeds = [
                VERIFIER_SET_TRACKER_SEED,
                verification_session_account.signature_verification
                .signing_verifier_set_hash.as_slice()
            ],
            bump = verifier_set_tracker_pda.bump
        )]
    pub verifier_set_tracker_pda: Account<'info, VerifierSetTracker>,
    #[account(
           init,
           payer = payer,
           space = VerifierSetTracker::DISCRIMINATOR.len() + std::mem::size_of::<VerifierSetTracker>(),
           seeds = [
               crate::seed_prefixes::VERIFIER_SET_TRACKER_SEED,
               new_verifier_set_merkle_root.as_ref()
           ],
           bump
       )]
    pub new_verifier_set_tracker: Account<'info, VerifierSetTracker>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
    pub operator: Option<Signer<'info>>,
}

pub fn rotate_signers_handler(
    ctx: Context<RotateSigners>,
    new_verifier_set_merkle_root: [u8; 32],
) -> Result<()> {
    // Check signature session is complete
    if !ctx
        .accounts
        .verification_session_account
        .signature_verification
        .is_valid()
    {
        return err!(GatewayError::SigningSessionNotValid);
    }

    // Check: we got the expected verifier hash
    if ctx.accounts.verifier_set_tracker_pda.verifier_set_hash
        != ctx
            .accounts
            .verification_session_account
            .signature_verification
            .signing_verifier_set_hash
    {
        return err!(GatewayError::InvalidVerifierSetTrackerProvided);
    }

    // Avoid rotating to already existing set
    if new_verifier_set_merkle_root == ctx.accounts.verifier_set_tracker_pda.verifier_set_hash {
        return err!(GatewayError::DuplicateVerifierSetRotation);
    }

    // Check current verifier set isn't expired
    let epoch = ctx.accounts.verifier_set_tracker_pda.epoch;
    ctx.accounts.gateway_root_pda.assert_valid_epoch(epoch)?;

    // we always enforce the delay unless the operator has been provided and
    // its also the Gateway operator
    // reference: https://github.com/axelarnetwork/axelar-gmp-sdk-solidity/blob/c290c7337fd447ecbb7426e52ac381175e33f602/contracts/gateway/AxelarAmplifierGateway.sol#L98-L101
    let operator = ctx.accounts.operator.clone();

    let enforce_rotation_delay = operator.map_or(true, |operator| {
        let operator_matches = *operator.key == ctx.accounts.gateway_root_pda.operator;
        let operator_is_signer = operator.is_signer;
        // if the operator matches and is also the signer - disable rotation delay
        !(operator_matches && operator_is_signer)
    });

    let is_latest =
        ctx.accounts.gateway_root_pda.current_epoch == ctx.accounts.verifier_set_tracker_pda.epoch;

    // Check: proof is signed by latest verifiers
    if enforce_rotation_delay && !is_latest {
        return err!(GatewayError::ProofNotSignedByLatestVerifierSet);
    }

    let current_time: u64 = solana_program::clock::Clock::get()?
        .unix_timestamp
        .try_into()
        .map_err(|_err| {
            solana_program::msg!("received negative timestamp");
            ProgramError::ArithmeticOverflow
        })?;

    if enforce_rotation_delay
        && !enough_time_till_next_rotation(current_time, &ctx.accounts.gateway_root_pda)
    {
        return err!(GatewayError::RotationCooldownNotDone);
    }

    ctx.accounts.gateway_root_pda.last_rotation_timestamp = current_time;

    rotate_signers(ctx, new_verifier_set_merkle_root)
}

fn rotate_signers(
    ctx: Context<RotateSigners>,
    new_verifier_set_merkle_root: [u8; 32],
) -> Result<()> {
    // Update Gateway config
    ctx.accounts.gateway_root_pda.current_epoch = ctx
        .accounts
        .gateway_root_pda
        .current_epoch
        .checked_add(U256::ONE)
        .ok_or(GatewayError::EpochCalculationOverflow)?;

    // Initialize the new verifier set tracker
    ctx.accounts.new_verifier_set_tracker.bump = ctx.bumps.new_verifier_set_tracker;
    ctx.accounts.new_verifier_set_tracker.epoch = ctx.accounts.gateway_root_pda.current_epoch;
    ctx.accounts.new_verifier_set_tracker.verifier_set_hash = new_verifier_set_merkle_root;

    emit_cpi!(VerifierSetRotatedEvent {
        verifier_set_hash: new_verifier_set_merkle_root,
        epoch: ctx.accounts.new_verifier_set_tracker.epoch,
    });

    Ok(())
}

pub fn construct_payload_hash(
    new_verifier_set_merkle_root: [u8; 32],
    signing_verifier_set_merkle_root: [u8; 32],
) -> [u8; 32] {
    const HASH_PREFIX: &[u8] = b"new verifier set";
    solana_program::keccak::hashv(&[
        HASH_PREFIX,
        &new_verifier_set_merkle_root,
        &signing_verifier_set_merkle_root,
    ])
    .to_bytes()
}

fn enough_time_till_next_rotation(current_time: u64, config: &GatewayConfig) -> bool {
    let secs_since_last_rotation = current_time
        .checked_sub(config.last_rotation_timestamp)
        .expect(
            "Current time minus rotate signers last successful operation time should not underflow",
        );
    secs_since_last_rotation >= config.minimum_rotation_delay
}
