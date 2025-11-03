use anchor_lang::prelude::*;
use anchor_lang::Discriminator;
use relayer_discovery::structs::{RelayerData, RelayerInstruction, RelayerTransaction};

use crate::instruction::GetTransaction;

#[derive(Accounts)]
pub struct Init<'info> {
    #[account(mut)]
    // The payer for the initialization.
    pub payer: Signer<'info>,

    #[account(
        mut,
        seeds = [
            &relayer_discovery::TRANSACTION_PDA_SEED,
        ],
        bump = relayer_discovery::find_transaction_pda(&crate::id()).1,
        constraint = relayer_transaction.key == &relayer_discovery::find_transaction_pda(&crate::id()).0,
    )]
    /// This account will store the RelayerTransaction for this program. I haven't figure out a way to initialize it here, because of the variable length of it, and the fact that the type is not defined in this crate.
    pub relayer_transaction: AccountInfo<'info>,

    // The system program.
    pub system_program: Program<'info, System>,
}

/// Initializes the relayer transaction for this executable.
pub fn init_handler(ctx: Context<Init>) -> Result<()> {
    // The relayer transaction to be stored. This should point to the `GetTransaction` entrypoint.
    RelayerTransaction::Discovery(RelayerInstruction {
        // We want the relayer to call this program.
        program_id: crate::ID,
        // No accounts are required for this.
        accounts: vec![],
        // The data we need to find the final transaction.
        data: vec![
            // We can easily get the discriminaator thankfully. Note that we need `instruction::GetTransaction` and not `instructions::GetTransaction`.
            RelayerData::Bytes(Vec::from(GetTransaction::DISCRIMINATOR)),
            // We do not want to prefix the payload with the length as it is decoded into a struct as opposed to a `Vec<u8>`.
            RelayerData::PayloadRaw,
            // The command id, which is the only thing required (alongside this crate's id) to derive all the accounts required by the gateway.
            RelayerData::CommandId,
        ],
    })
    .init(
        &crate::id(),
        &ctx.accounts.system_program.to_account_info(),
        &ctx.accounts.payer.to_account_info(),
        &ctx.accounts.relayer_transaction,
    )?;
    Ok(())
}
