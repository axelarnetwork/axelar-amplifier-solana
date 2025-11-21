use anchor_lang::solana_program::instruction::Instruction;
use anchor_lang::{prelude::*, InstructionData};

use crate::Counter;

#[derive(Accounts)]
pub struct Init<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    // The counter account
    #[account(
        init,
        space = Counter::DISCRIMINATOR.len() + Counter::INIT_SPACE,
        payer = payer,
        seeds = [Counter::SEED_PREFIX],
        bump
    )]
    pub counter: Account<'info, Counter>,

    pub system_program: Program<'info, System>,
}

pub fn init_handler(ctx: Context<Init>) -> Result<()> {
    ctx.accounts.counter.bump = ctx.bumps.counter;

    Ok(())
}

pub fn make_init_ix(payer: Pubkey) -> Instruction {
    let counter = Pubkey::find_program_address(&[Counter::SEED_PREFIX], &crate::ID).0;

    let accounts = crate::accounts::Init {
        payer,
        counter,
        system_program: anchor_lang::system_program::ID,
    };

    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(None),
        data: crate::instruction::Init {}.data(),
    }
}
