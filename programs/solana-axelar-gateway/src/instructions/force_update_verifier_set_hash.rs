use crate::VerifierSetTracker;
use anchor_lang::prelude::*;

// Temporary hardcoded verifier set hash to write into the tracker PDA.
// Update this constant as needed.
const FORCED_VERIFIER_SET_HASH: [u8; 32] = [
    63, 204, 176, 41, 93, 75, 19, 15, 175, 173, 159, 241, 66, 139, 44, 0, 236, 116, 204, 156, 38,
    21, 242, 171, 33, 206, 75, 7, 251, 211, 16, 181,
];

#[derive(Accounts)]
pub struct ForceUpdateVerifierSetHash<'info> {
    #[account(mut)]
    pub verifier_set_tracker_pda: AccountLoader<'info, VerifierSetTracker>,
}

pub fn force_update_verifier_set_hash_handler(
    ctx: Context<ForceUpdateVerifierSetHash>,
) -> Result<()> {
    let mut tracker = ctx.accounts.verifier_set_tracker_pda.load_mut()?;
    tracker.verifier_set_hash = FORCED_VERIFIER_SET_HASH;
    Ok(())
}
