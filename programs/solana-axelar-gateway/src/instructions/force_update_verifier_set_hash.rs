use crate::{GatewayConfig, VerifierSetTracker};
use anchor_lang::prelude::*;

// Temporary hardcoded verifier set hash to write into the tracker PDA.
// Update this constant as needed.
const OLD_VERIFIER_SET_HASH: [u8; 32] = [
    40, 242, 108, 137, 95, 154, 208, 193, 184, 47, 73, 20, 243, 252, 89, 233, 92, 110, 132, 233,
    232, 174, 15, 129, 213, 101, 11, 166, 244, 16, 191, 184,
];

const NEW_VERIFIER_SET_HASH: [u8; 32] = [
    63, 204, 176, 41, 93, 75, 19, 15, 175, 173, 159, 241, 66, 139, 44, 0, 236, 116, 204, 156, 38,
    21, 242, 171, 33, 206, 75, 7, 251, 211, 16, 181,
];

#[derive(Accounts)]
pub struct ForceUpdateVerifierSetHash<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        seeds = [GatewayConfig::SEED_PREFIX],
        bump = gateway_root_pda.load()?.bump
    )]
    pub gateway_root_pda: AccountLoader<'info, GatewayConfig>,

    #[account(mut)]
    pub old_verifier_set_tracker_pda: AccountLoader<'info, VerifierSetTracker>,

    #[account(
        init,
        payer = payer,
        space = VerifierSetTracker::DISCRIMINATOR.len() + std::mem::size_of::<VerifierSetTracker>(),
        seeds = [
            VerifierSetTracker::SEED_PREFIX,
            NEW_VERIFIER_SET_HASH.as_ref()
        ],
        bump
    )]
    pub new_verifier_set_tracker_pda: AccountLoader<'info, VerifierSetTracker>,

    pub system_program: Program<'info, System>,
}

pub fn force_update_verifier_set_hash_handler(
    ctx: Context<ForceUpdateVerifierSetHash>,
) -> Result<()> {
    let mut old_tracker = ctx.accounts.old_verifier_set_tracker_pda.load_mut()?;
    let mut new_tracker = ctx.accounts.new_verifier_set_tracker_pda.load_init()?;

    old_tracker.verifier_set_hash = OLD_VERIFIER_SET_HASH;
    new_tracker.bump = ctx.bumps.new_verifier_set_tracker_pda;
    new_tracker.epoch = ctx.accounts.gateway_root_pda.load()?.current_epoch;
    new_tracker.verifier_set_hash = NEW_VERIFIER_SET_HASH;
    Ok(())
}
