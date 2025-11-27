use anchor_lang::prelude::*;
use anchor_lang::Discriminator;
use relayer_discovery::structs::{RelayerData, RelayerInstruction, RelayerTransaction};
use relayer_discovery::transaction_pda_accounts;

use crate::instruction::GetTransaction;

transaction_pda_accounts!(relayer_transaction());

#[derive(Accounts)]
pub struct Init<'info> {
    transaction: RelayerTransactionAccounts<'info>,
}

/// Initializes the relayer transaction for this executable.
pub fn init_handler(ctx: Context<Init>) -> Result<()> {
    relayer_transaction().serialize(
        &mut &mut ctx
            .accounts
            .transaction
            .relayer_transaction
            .data
            .borrow_mut()[..],
    )?;

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
