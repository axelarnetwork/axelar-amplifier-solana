use anchor_lang::prelude::*;
use relayer_discovery::structs::{RelayerData, RelayerInstruction, RelayerTransaction};
use anchor_lang::Discriminator;

use crate::instruction::GetTransaction;

#[derive(Accounts)]
pub struct Init<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        mut,
        seeds = [
            &relayer_discovery::TRANSACTION_PDA_SEED,
        ],
        bump = relayer_discovery::find_transaction_pda(&crate::id()).1,
        constraint = relayer_transaction.key == &relayer_discovery::find_transaction_pda(&crate::id()).0
    )]
    /// This account will store the RelayerTransaction for this program.
    pub relayer_transaction: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
}

pub fn init_handler(ctx: Context<Init>) -> Result<()> {
    
    relayer_transaction().init(
        &crate::id(),
        &ctx.accounts.system_program.to_account_info(),
        &ctx.accounts.payer.to_account_info(),
        &ctx.accounts.relayer_transaction,
    )?;
    Ok(())
}

fn relayer_transaction() -> RelayerTransaction {
    RelayerTransaction::Discovery(RelayerInstruction {
        program_id: crate::ID,
        accounts: vec![
        ],
        data: vec![
            RelayerData::Bytes(Vec::from(GetTransaction::DISCRIMINATOR)),
            RelayerData::PayloadRaw,
            RelayerData::CommandId,
        ],
    })
}
