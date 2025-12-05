use crate::{state::InterchainTokenService, ItsError};
use anchor_lang::prelude::*;
#[allow(deprecated)]
use anchor_lang::solana_program::bpf_loader_upgradeable;
use anchor_lang::solana_program::instruction::Instruction;
use anchor_lang::InstructionData;

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

/// Creates a SetPauseStatus instruction
pub fn make_set_pause_status_instruction(
    payer: Pubkey,
    paused: bool,
) -> (Instruction, crate::accounts::SetPauseStatus) {
    let its_root_pda = InterchainTokenService::find_pda().0;

    let program_data = bpf_loader_upgradeable::get_program_data_address(&crate::ID);

    let accounts = crate::accounts::SetPauseStatus {
        payer,
        program_data,
        its_root_pda,
    };

    (
        Instruction {
            program_id: crate::ID,
            accounts: accounts.to_account_metas(None),
            data: crate::instruction::SetPauseStatus { paused }.data(),
        },
        accounts,
    )
}
