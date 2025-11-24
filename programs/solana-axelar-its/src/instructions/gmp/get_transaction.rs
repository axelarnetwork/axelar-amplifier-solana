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
#[instruction(message: Message, payload: Vec<u8>)]
pub struct GetTransaction<'info> {    
    #[account(
        seeds = [
            TokenManager::SEED_PREFIX,
            InterchainTokenService::find_pda().0.key().as_ref(),
            GMPPayload::decode(&payload).unwrap().token_id().unwrap().as_ref(),
        ]
        ,
        bump = token_manager_pda.bump,
    )]
    pub token_manager_pda: Option<Account<'info, TokenManager>>,
}

/// This should return a `RelayerTransaction` that will convert to an `Execute` instruction properly, for a given `payload` and `command_id`. No accounts are needed to find this information.
///
pub fn get_transaction_handler(
    ctx: Context<GetTransaction>,
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
        InterchainTransfer(transfer) => insterchain_transfer_transaction(message, transfer, inner_msg.source_chain, ctx.accounts.token_manager_pda.take()),
        DeployInterchainToken(deploy) => deploy_interchain_token_transaction(message, deploy, inner_msg.source_chain),
        LinkToken(link_token) => link_token_transaction(message, link_token, inner_msg.source_chain),
        _ => {
            err!(ItsError::InvalidInstructionData)
        }
    }
}

fn deploy_interchain_token_transaction(message: Message, deploy: DeployInterchainToken, source_chain: String) -> Result<RelayerTransaction> {
    let program_id = crate::ID;
    let token_id = deploy.token_id.0;
    let minter = deploy.minter.to_vec();
    let name = deploy.name;
    let symbol = deploy.symbol;
    let decimals = deploy.decimals;
    
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
        vec![
            RelayerAccount::Account {
                pubkey: program_id,
                is_writable: false
            },
            RelayerAccount::Account {
                pubkey: program_id,
                is_writable: false
            },
        ]
    } else {
        let minter = Pubkey::try_from(minter.clone()).map_err(|_| ItsError::InvalidArgument)?;
        let minter_roles = UserRoles::find_pda(&token_manager_pda, &minter).0;
        vec![
            RelayerAccount::Account {
                pubkey: minter,
                is_writable: false
            },
            RelayerAccount::Account {
                pubkey: minter_roles,
                is_writable: true
            },
        ]
    };
    
    let (event_authority, _) = Pubkey::find_program_address(&[b"__event_authority"], &program_id);
    
    Ok(RelayerTransaction::Final(
        // A single instruction is required. Note that we could be fancy and check whether the counter_pda is initialized (which would required one more discovery transaction be performed),
        // And only if it is not initialized prepend a transaction that initializes it. Then we could omit the `payer` and `system_program` accounts from the actual execute instruction.
        vec![RelayerInstruction {
            // We want this program to be the entrypoint.
            program_id,
            // The accounts needed.
            accounts: [
                vec![
                    // payer, need to do testing to figure out the amount needed here.
                    RelayerAccount::Payer(1000000000),
                    // system_program,
                    RelayerAccount::Account {
                        pubkey: system_program::ID,
                        is_writable: false,
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
                vec![
                    RelayerAccount::Account { pubkey: event_authority, is_writable: false },
                    RelayerAccount::Account { pubkey: program_id, is_writable: false },
                ]
            ]
            .concat(),
            // The data needed.
            data: vec![
                // The discriminator
                RelayerData::Bytes(Vec::from(instruction::ExecuteDeployInterchainToken::DISCRIMINATOR)),
                RelayerData::from_serializable(token_id)?,
                RelayerData::from_serializable(name)?,
                RelayerData::from_serializable(symbol)?,
                RelayerData::from_serializable(decimals)?,
                RelayerData::from_serializable(minter)?,
                RelayerData::from_serializable(source_chain)?,
                RelayerData::Message,
            ],
        }],
    ))
}

fn link_token_transaction(message: Message, link_token: LinkToken, source_chain: String) -> Result<RelayerTransaction> {
    let program_id = crate::ID;
    let token_id = link_token.token_id.0;
    let token_mint_pda = Pubkey::new_from_array(link_token.destination_token_address.as_ref().try_into().map_err(|_| ItsError::InvalidInstructionData)?);
    let token_manager_type: u8 = link_token.token_manager_type.try_into().map_err(|_| ItsError::InvalidArgument)?;
    
    let its_root_pda = InterchainTokenService::find_pda().0;
    let token_manager_pda = TokenManager::find_pda(token_id, its_root_pda).0;
    let token_manager_ata = get_associated_token_address_with_program_id(
        &token_manager_pda,
        &token_mint_pda,
        &spl_token_2022::id(),
    );
    let token_program = anchor_spl::token_2022::spl_token_2022::id();
    let associated_token_program = anchor_spl::associated_token::spl_associated_token_account::id();

    let operator_accounts = if link_token.link_params.is_empty() {
        vec![
            RelayerAccount::Account {
                pubkey: program_id,
                is_writable: false
            },
            RelayerAccount::Account {
                pubkey: program_id,
                is_writable: false
            },
        ]
    } else {
        let operator = Pubkey::try_from(link_token.link_params.to_vec()).map_err(|_| ItsError::InvalidInstructionData)?;
        let operator_roles = UserRoles::find_pda(&token_manager_pda, &operator).0;
        vec![
            RelayerAccount::Account {
                pubkey: operator,
                is_writable: false
            },
            RelayerAccount::Account {
                pubkey: operator_roles,
                is_writable: true
            },
        ]
    };
    
    let (event_authority, _) = Pubkey::find_program_address(&[b"__event_authority"], &program_id);

    Ok(RelayerTransaction::Final(
        // A single instruction is required. Note that we could be fancy and check whether the counter_pda is initialized (which would required one more discovery transaction be performed),
        // And only if it is not initialized prepend a transaction that initializes it. Then we could omit the `payer` and `system_program` accounts from the actual execute instruction.
        vec![RelayerInstruction {
            // We want this program to be the entrypoint.
            program_id,
            // The accounts needed.
            accounts: [
                vec![
                    // payer, need to do testing to figure out the amount needed here.
                    RelayerAccount::Payer(1000000000),
                    // system_program,
                    RelayerAccount::Account {
                        pubkey: system_program::ID,
                        is_writable: false,
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
                ],
                operator_accounts,
                vec![
                    RelayerAccount::Account { pubkey: event_authority, is_writable: false },
                    RelayerAccount::Account { pubkey: program_id, is_writable: false },
                ]
            ]
            .concat(),
            // The data needed.
            data: vec![
                // The discriminator
                RelayerData::Bytes(Vec::from(instruction::LinkToken::DISCRIMINATOR)),
                RelayerData::from_serializable(token_id)?,
                RelayerData::from_serializable(token_manager_type)?,
                RelayerData::from_serializable(link_token.source_token_address.to_vec())?,
                RelayerData::from_serializable(link_token.destination_token_address.to_vec())?,
                RelayerData::from_serializable(link_token.link_params.to_vec())?,
                RelayerData::from_serializable(source_chain)?,
                RelayerData::Message,
            ],
        }],
    ))
}

fn insterchain_transfer_transaction<'info>(message: Message, transfer: InterchainTransfer, source_chain: String, token_manager: Option<Account<'info, TokenManager>>) -> Result<RelayerTransaction> {
    let Some(token_manager) = token_manager
    else {
        return Ok(RelayerTransaction::Discovery(RelayerInstruction {
            // We want the relayer to call this program.
            program_id: crate::ID,
            // No accounts are required for this.
            accounts: vec![
                RelayerAccount::Account { 
                    pubkey: TokenManager::find_pda(transfer.token_id.0, InterchainTokenService::find_pda().0).0,
                    is_writable: false,
                },
            ],
            // The data we need to find the final transaction.
            data: vec![
                RelayerData::Bytes(Vec::from(instruction::GetTransaction::DISCRIMINATOR)),
                RelayerData::Message,
                RelayerData::Payload,
            ],
        }));
    };

    let token_id = transfer.token_id.0;
    let source_address = transfer.source_address.to_vec();
    let destination_address = Pubkey::try_from(transfer.destination_address.to_vec()).map_err(|_| ItsError::InvalidArgument)?;
    let amount: u64 = transfer.amount.try_into().map_err(|_| ItsError::ArithmeticOverflow)?;
    let data = transfer.data.to_vec();

    let program_id = crate::ID;
    let token_mint_pda = token_manager.token_address;
    let destination_pda = Pubkey::try_from(transfer.destination_address.to_vec()).map_err(|_| ItsError::InvalidArgument)?;
    let destination_ata = get_associated_token_address_with_program_id(
        &destination_pda,
        &token_mint_pda,
        &spl_token_2022::id(),
    );
    
    let its_root_pda = InterchainTokenService::find_pda().0;
    let token_manager_ata = get_associated_token_address_with_program_id(
        &token_manager.key(),
        &token_mint_pda,
        &spl_token_2022::id(),
    );
    let token_program = anchor_spl::token_2022::spl_token_2022::id();
    let associated_token_program = anchor_spl::associated_token::spl_associated_token_account::id();
    let (event_authority, _) = Pubkey::find_program_address(&[b"__event_authority"], &program_id);

    Ok(RelayerTransaction::Final(
        // A single instruction is required. Note that we could be fancy and check whether the counter_pda is initialized (which would required one more discovery transaction be performed),
        // And only if it is not initialized prepend a transaction that initializes it. Then we could omit the `payer` and `system_program` accounts from the actual execute instruction.
        vec![RelayerInstruction {
            // We want this program to be the entrypoint.
            program_id,
            // The accounts needed.
            accounts: [
                vec![
                    // payer, need to do testing to figure out the amount needed here.
                    RelayerAccount::Payer(1000000000),
                ],
                // executable
                relayer_discovery::executable_relayer_accounts(&message.command_id(), &crate::id()),
                vec![
                    RelayerAccount::Account {
                        pubkey: its_root_pda,
                        is_writable: false
                    },
                    RelayerAccount::Account {
                        pubkey: destination_pda,
                        is_writable: false
                    },
                    RelayerAccount::Account {
                        pubkey: destination_ata,
                        is_writable: true
                    },
                    RelayerAccount::Account {
                        pubkey: token_mint_pda,
                        is_writable: true
                    },
                    RelayerAccount::Account {
                        pubkey: token_manager.key(),
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
                        pubkey: system_program::ID,
                        is_writable: false
                    },
                ],
                vec![
                    RelayerAccount::Account { pubkey: event_authority, is_writable: false },
                    RelayerAccount::Account { pubkey: program_id, is_writable: false },
                ]
            ]
            .concat(),
            // The data needed.
            data: vec![
                // The discriminator
                RelayerData::Bytes(Vec::from(instruction::ExecuteInterchainTransfer::DISCRIMINATOR)),
                RelayerData::from_serializable(token_id)?,
                RelayerData::from_serializable(source_address)?,
                RelayerData::from_serializable(destination_address)?,
                RelayerData::from_serializable(amount)?,
                RelayerData::from_serializable(data)?,
                RelayerData::from_serializable(source_chain)?,
                RelayerData::Message,
            ],
        }],
    ))
}
