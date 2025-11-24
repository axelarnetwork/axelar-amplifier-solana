#![allow(clippy::missing_asserts_for_indexing)]
use crate::instruction::GetTransaction;
use anchor_lang::prelude::*;
use relayer_discovery::{structs::{RelayerAccount, RelayerData, RelayerInstruction, RelayerTransaction}, transaction_pda_accounts};


transaction_pda_accounts!(relayer_transaction);

/// Initialize the configuration PDA.
#[derive(Accounts)]
pub struct RegisterDiscoveryTransaction<'info> {
    transaction: RelayerTransactionAccounts<'info>,
}

pub fn register_discovery_transaction(
    ctx: Context<RegisterDiscoveryTransaction>,
) -> Result<()> {
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

pub fn relayer_transaction() -> RelayerTransaction {
    RelayerTransaction::Discovery(RelayerInstruction {
        // We want the relayer to call this program.
        program_id: crate::ID,
        // No accounts are required for this.
        accounts: vec![
            RelayerAccount::Account { 
                pubkey: crate::ID,
                is_writable: false,
            },
        ],
        // The data we need to find the final transaction.
        data: vec![
            // We can easily get the discriminaator thankfully. Note that we need `instruction::GetTransaction` and not `instructions::GetTransaction`.
            RelayerData::Bytes(Vec::from(GetTransaction::DISCRIMINATOR)),
            // We do not want to prefix the payload with the length as it is decoded into a struct as opposed to a `Vec<u8>`.
            RelayerData::Message,
            // The command id, which is the only thing required (alongside this crate's id) to derive all the accounts required by the gateway.
            RelayerData::Payload,
        ],
    })
}