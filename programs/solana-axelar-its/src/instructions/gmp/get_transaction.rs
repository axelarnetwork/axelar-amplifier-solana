use anchor_spl::{associated_token::get_associated_token_address_with_program_id, token_2022::spl_token_2022};
use interchain_token_transfer_gmp::{DeployInterchainToken, GMPPayload, InterchainTransfer, LinkToken};  
use anchor_lang::{prelude::*, system_program};
use mpl_token_metadata::accounts::Metadata;
use relayer_discovery::structs::{
    RelayerAccount, RelayerData, RelayerInstruction, RelayerTransaction,
};
use solana_axelar_gateway::Message;
use crate::{errors::ItsError, instruction, state::{InterchainTokenService, TokenManager, UserRoles}};

#[derive(Accounts)]
pub struct GetTransaction<'info> {
    #[account(mut)]
    pub payer: Option<Signer<'info>>,
}

/// This should return a `RelayerTransaction` that will convert to an `Execute` instruction properly, for a given `payload` and `command_id`. No accounts are needed to find this information.
///
pub fn get_transaction_handler(
    _: Context<GetTransaction>,
    message: Message,
    payload: Vec<u8>,
) -> Result<RelayerTransaction> {
    use GMPPayload::{InterchainTransfer, DeployInterchainToken, LinkToken, ReceiveFromHub};

    // Execute can only be called with ReceiveFromHub payload at the top level
    let ReceiveFromHub(inner_msg) =
        GMPPayload::decode(&payload).map_err(|_err| ItsError::InvalidInstructionData)?
    else {
        msg!("Unsupported GMP payload");
        return err!(ItsError::InvalidInstructionData);
    };

    let payload = GMPPayload::decode(&inner_msg.payload).map_err(|_err| ItsError::InvalidInstructionData)?;

    match payload {
        InterchainTransfer(transfer) => insterchain_transfer_transaction(message, transfer),
        DeployInterchainToken(deploy) => deploy_interchain_token_transaction(message, deploy),
        LinkToken(payload) => link_token_transaction(message, payload),
        _ => {
            err!(ItsError::InvalidInstructionData)
        }
    }
}

fn deploy_interchain_token_transaction(message: Message, deploy: DeployInterchainToken) -> Result<RelayerTransaction> {
    let token_id = deploy.token_id.0;
    let minter = deploy.minter.to_vec();
    
    let its_root_pda = InterchainTokenService::find_pda().0;
    let token_manager_pda = TokenManager::find_pda(token_id, its_root_pda).0;
    let token_mint_pda = TokenManager::find_token_mint_pda(token_id, its_root_pda).0;
    let token_manager_ata = get_associated_token_address_with_program_id(
        &token_manager_pda,
        &token_mint_pda,
        &spl_token_2022::id(),
    );
    let token_program = anchor_spl::token_2022::spl_token_2022::id();
    let associated_token_program = anchor_spl::associated_token::spl_associated_token_account::id();
    let sysvar_instructions = anchor_lang::solana_program::sysvar::instructions::id();
    let mpl_token_metadata = mpl_token_metadata::programs::MPL_TOKEN_METADATA_ID;
    let mpl_token_metadata_account = Metadata::find_pda(&token_mint_pda).0;
    let minter_accounts = if minter.is_empty() {
        vec![]
    } else {
        let minter = Pubkey::try_from(minter).map_err(|_| ItsError::MinterConversionFailed)?;
        let minter_roles = UserRoles::find_pda(&token_manager_pda, &minter).0;
        vec![
            RelayerAccount::Account {
                pubkey: minter,
                is_writable: false
            },
            RelayerAccount::Account {
                pubkey: minter_roles,
                is_writable: false
            },
        ]
    };
    
    Ok(RelayerTransaction::Final(
        // A single instruction is required. Note that we could be fancy and check whether the counter_pda is initialized (which would required one more discovery transaction be performed),
        // And only if it is not initialized prepend a transaction that initializes it. Then we could omit the `payer` and `system_program` accounts from the actual execute instruction.
        vec![RelayerInstruction {
            // We want this program to be the entrypoint.
            program_id: crate::id(),
            // The accounts needed.
            accounts: [
                vec![
                    // payer, need to do testing to figure out the amount needed here.
                    RelayerAccount::Payer(1000),
                    // system_program,
                    RelayerAccount::Account {
                        pubkey: system_program::ID,
                        is_writable: true,
                    },
                ],
                // executable
                relayer_discovery::executable_relayer_accounts(&message.command_id(), &crate::id()),
                vec![
                    RelayerAccount::Account {
                        pubkey: its_root_pda,
                        is_writable: false
                    },
                    RelayerAccount::Account {
                        pubkey: token_manager_pda,
                        is_writable: true
                    },
                    RelayerAccount::Account {
                        pubkey: token_mint_pda,
                        is_writable: true
                    },
                    RelayerAccount::Account {
                        pubkey: token_manager_ata,
                        is_writable: true
                    },
                    RelayerAccount::Account {
                        pubkey: token_program,
                        is_writable: false
                    },
                    RelayerAccount::Account {
                        pubkey: associated_token_program,
                        is_writable: false
                    },
                    RelayerAccount::Account {
                        pubkey: sysvar_instructions,
                        is_writable: false
                    },
                    RelayerAccount::Account {
                        pubkey: mpl_token_metadata,
                        is_writable: false
                    },
                    RelayerAccount::Account {
                        pubkey: mpl_token_metadata_account,
                        is_writable: true
                    },
                ],
                minter_accounts,
            ]
            .concat(),
            // The data needed.
            data: vec![
                // The discriminator
                RelayerData::Bytes(Vec::from(instruction::DeployInterchainToken::DISCRIMINATOR)),
                // The message, which is needed for the gateway.
                RelayerData::Message,
                // We want to prefix the payload with the length as it is decoded into a `Vec<u8>`.
                RelayerData::Payload,
            ],
        }],
    ))
}

fn link_token_transaction(message: Message, payload: LinkToken) -> Result<RelayerTransaction> {
    Ok(RelayerTransaction::Final(
        // A single instruction is required. Note that we could be fancy and check whether the counter_pda is initialized (which would required one more discovery transaction be performed),
        // And only if it is not initialized prepend a transaction that initializes it. Then we could omit the `payer` and `system_program` accounts from the actual execute instruction.
        vec![RelayerInstruction {
            // We want this program to be the entrypoint.
            program_id: crate::id(),
            // The accounts needed.
            accounts: [
                // First we need the executable accounts.
                relayer_discovery::executable_relayer_accounts(&message.command_id(), &crate::id()),
                // Followed by the accounts needed to modify storage of the executable.
                vec![
                ],
            ]
            .concat(),
            // The data needed.
            data: vec![
            ],
        }],
    ))
}

fn insterchain_transfer_transaction(message: Message, transfer: InterchainTransfer) -> Result<RelayerTransaction> {
    Ok(RelayerTransaction::Final(
        // A single instruction is required. Note that we could be fancy and check whether the counter_pda is initialized (which would required one more discovery transaction be performed),
        // And only if it is not initialized prepend a transaction that initializes it. Then we could omit the `payer` and `system_program` accounts from the actual execute instruction.
        vec![RelayerInstruction {
            // We want this program to be the entrypoint.
            program_id: crate::id(),
            // The accounts needed.
            accounts: [
                // First we need the executable accounts.
                relayer_discovery::executable_relayer_accounts(&message.command_id(), &crate::id()),
                // Followed by the accounts needed to modify storage of the executable.
                vec![
                ],
            ]
            .concat(),
            // The data needed.
            data: vec![
            ],
        }],
    ))
}