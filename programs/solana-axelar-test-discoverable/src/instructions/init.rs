#![allow(clippy::missing_asserts_for_indexing)]
use anchor_lang::prelude::*;
use anchor_lang::solana_program::instruction::Instruction;
use anchor_lang::system_program;
use anchor_lang::Discriminator;
use anchor_lang::InstructionData;
use relayer_discovery::structs::RelayerAccount;
use relayer_discovery::structs::{RelayerData, RelayerInstruction, RelayerTransaction};
use relayer_discovery::transaction_pda_accounts;
use relayer_discovery::TRANSACTION_PDA_SEED;
use solana_axelar_its::seed_prefixes::INTERCHAIN_EXECUTABLE_TRANSACTION_PDA_SEED;

use crate::instruction::{GetItsTransaction, GetTransaction};

#[derive(Accounts)]
pub struct Init<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    // IncomingMessage PDA account
    // needs to be mutable as the validate_message CPI
    // updates its state
    #[account(
        init,
        seeds = [TRANSACTION_PDA_SEED],
        bump,
        payer = payer,
        space = {
            let mut bytes = Vec::with_capacity(256);
            relayer_transaction().serialize(&mut bytes)?;
            bytes.len()
        }
    )]
    pub transaction: AccountInfo<'info>,

    #[account(
        init,
        seeds = [INTERCHAIN_EXECUTABLE_TRANSACTION_PDA_SEED],
        bump,
        payer = payer,
        space = {
            let mut bytes = Vec::with_capacity(256);
            its_relayer_transaction().serialize(&mut bytes)?;
            bytes.len()
        }
    )]
    pub its_transaction: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
}

/// Initializes the relayer transaction for this executable.
pub fn init_handler(ctx: Context<Init>) -> Result<()> {
    relayer_transaction().serialize(&mut &mut ctx.accounts.transaction.data.borrow_mut()[..])?;

    its_relayer_transaction()
        .serialize(&mut &mut ctx.accounts.its_transaction.data.borrow_mut()[..])?;

    Ok(())
}

fn relayer_transaction() -> RelayerTransaction {
    RelayerTransaction::Discovery(RelayerInstruction {
        // We want the relayer to call this program.
        program_id: crate::ID,
        // No accounts are required for this.
        accounts: vec![RelayerAccount::Account {
            pubkey: crate::ID,
            is_writable: false,
        }],
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

fn its_relayer_transaction() -> RelayerTransaction {
    RelayerTransaction::Discovery(RelayerInstruction {
        // We want the relayer to call this program.
        program_id: crate::ID,
        // No accounts are required for this.
        accounts: vec![],
        // The data we need to find the final transaction.
        data: vec![
            // We can easily get the discriminaator thankfully. Note that we need `instruction::GetTransaction` and not `instructions::GetTransaction`.
            RelayerData::Bytes(Vec::from(GetItsTransaction::DISCRIMINATOR)),
            // We do not want to prefix the payload with the length as it is decoded into a struct as opposed to a `Vec<u8>`.
            RelayerData::Message,
            // The command id, which is the only thing required (alongside this crate's id) to derive all the accounts required by the gateway.
            RelayerData::Payload,
        ],
    })
}

pub fn make_init_ix(payer: Pubkey) -> Instruction {
    let accounts = crate::accounts::Init {
        payer,
        transaction: relayer_discovery::find_transaction_pda(&crate::ID).0,
        system_program: system_program::ID,
        its_transaction: solana_axelar_its::utils::find_interchain_executable_transaction_pda(
            &crate::ID,
        )
        .0,
    };

    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(None),
        data: crate::instruction::Init {}.data(),
    }
}
