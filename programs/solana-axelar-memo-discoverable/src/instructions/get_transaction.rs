use crate::{instruction::Execute, Counter, Payload};
use anchor_lang::{prelude::*, system_program};
use relayer_discovery::structs::{
    RelayerAccount, RelayerData, RelayerInstruction, RelayerTransaction,
};

#[derive(Accounts)]
#[instruction(payload: Payload)]
pub struct GetTransaction {}

/// This should return a `RelayerTransaction` that will convert to an `Execute` instruction properly, for a given `payload` and `command_id`. No accounts are needed to find this information.
///
pub fn get_transaction_handler(
    _: Context<GetTransaction>,
    payload: Payload,
    command_id: [u8; 32],
) -> Result<RelayerTransaction> {
    let counter_pda = Counter::get_pda(payload.storage_id).0;
    Ok(RelayerTransaction::Final(
        // A single instruction is required. Note that we could be fancy and check whether the counter_pda is initialized (which would required one more discovery transaction be performed),
        // And only if it is not initialized prepend a transaction that initializes it. Then we could omit the `payer` and `system_program` accounts from the actual execute instruction.
        vec![RelayerInstruction {
            // We want this program to be the entrypoint.
            program_id: crate::id(),
            // The accounts needed.
            accounts: [
                // First we need the executable accounts.
                relayer_discovery::executable_relayer_accounts(&command_id, &crate::id()),
                // Followed by the accounts needed to modify storage of the executable.
                vec![
                    RelayerAccount::Payer(1_000_000_000),
                    RelayerAccount::Account {
                        pubkey: counter_pda,
                        is_writable: true,
                    },
                    RelayerAccount::Account {
                        pubkey: system_program::ID,
                        is_writable: false,
                    },
                ],
            ]
            .concat(),
            // The data needed.
            data: vec![
                // We can easily get the discriminaator thankfully. Note that we need `instruction::Execute` and not `instructions::Execute`.
                RelayerData::Bytes(Vec::from(Execute::DISCRIMINATOR)),
                // We do not want to prefix the payload with the length as it is decoded into a struct as opposed to a `Vec<u8>`.
                RelayerData::PayloadRaw,
                // The message, which is needed for the gateway.
                RelayerData::Message,
            ],
        }],
    ))
}
