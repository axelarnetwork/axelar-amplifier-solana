use anchor_lang::prelude::*;
use anchor_lang::Discriminator;
use relayer_discovery::structs::{RelayerData, RelayerInstruction, RelayerTransaction};

use crate::instruction::GetTransaction;

#[derive(Accounts)]
pub struct Init<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    // IncomingMessage PDA account
    // needs to be mutable as the validate_message CPI
    // updates its state
    #[account(
        init,
        seeds = [relayer_discovery::TRANSACTION_PDA_SEED],
        bump,
        payer = payer,
        space = {
            let mut bytes = Vec::with_capacity(256);
            relayer_transaction().serialize(&mut bytes)?;
            bytes.len()
        }
    )]
    pub transaction: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
}

/// Initializes the relayer transaction for this executable.
pub fn init_handler(ctx: Context<Init>) -> Result<()> {
    relayer_transaction().serialize(&mut &mut ctx.accounts.transaction.data.borrow_mut()[..])?;

    Ok(())
}

fn relayer_transaction() -> RelayerTransaction {
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
}
