#![allow(clippy::missing_asserts_for_indexing)]
use crate::{InterchainTokenService, TokenManager, instruction::GetTransaction, utils::relayer_transaction};
use anchor_lang::prelude::*;
use relayer_discovery::{
    structs::{RelayerAccount, RelayerData, RelayerInstruction, RelayerTransaction},
    transaction_pda_accounts,
};

transaction_pda_accounts!(relayer_transaction(None, None));

/// Initialize the configuration PDA.
#[derive(Accounts)]
pub struct RegisterDiscoveryTransaction<'info> {
    transaction: RelayerTransactionAccounts<'info>,
}

pub fn register_discovery_transaction(ctx: Context<RegisterDiscoveryTransaction>) -> Result<()> {
    relayer_transaction(None, None).serialize(
        &mut &mut ctx
            .accounts
            .transaction
            .relayer_transaction
            .data
            .borrow_mut()[..],
    )?;

    Ok(())
}