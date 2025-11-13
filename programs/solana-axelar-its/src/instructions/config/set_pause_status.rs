use crate::{state::InterchainTokenService, ItsError};
use anchor_lang::prelude::*;
#[allow(deprecated)]
use anchor_lang::solana_program::bpf_loader_upgradeable;

#[derive(Accounts)]
#[instruction(paused: bool)]
pub struct SetPauseStatus<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        seeds = [crate::ID.as_ref()],
        bump,
        seeds::program = bpf_loader_upgradeable::ID,
        constraint = program_data.upgrade_authority_address == Some(payer.key())
            @ ItsError::InvalidAccountData
    )]
    pub program_data: Account<'info, ProgramData>,

    #[account(
    	mut,
     	seeds = [InterchainTokenService::SEED_PREFIX],
     	bump = its_root_pda.bump,
      	constraint = its_root_pda.paused != paused @ ItsError::InvalidArgument,
    )]
    pub its_root_pda: Account<'info, InterchainTokenService>,
}

pub fn set_pause_status(ctx: Context<SetPauseStatus>, paused: bool) -> Result<()> {
    ctx.accounts.its_root_pda.paused = paused;

    Ok(())
}
