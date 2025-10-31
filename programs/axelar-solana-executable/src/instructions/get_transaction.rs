use anchor_lang::{prelude::*, system_program};
use axelar_solana_gateway_v2::{GatewayConfig, IncomingMessage, ID as GATEWAY_PROGRAM_ID};
use relayer_discovery::structs::{RelayerAccount, RelayerData, RelayerInstruction, RelayerTransaction};
use crate::{Counter, Payload, instruction::Execute};

#[derive(Accounts)]
#[instruction(payload: Payload)]
pub struct GetTransaction {
}

pub fn get_transaction_handler(_: Context<GetTransaction>, payload: Payload, command_id: [u8; 32]) -> Result<RelayerTransaction> {
    let incoming_message = IncomingMessage::find_pda(&command_id).0;
    let signing_pda = IncomingMessage::find_signing_pda(&command_id, &crate::id()).0;
    let gateway_root_pda = GatewayConfig::find_pda().0;
    let event_authority = Pubkey::find_program_address(&[b"__event_authority"], &GATEWAY_PROGRAM_ID).0;
    let counter_pda = Counter::get_pda(payload.storage_id).0;
    Ok(RelayerTransaction::Final(vec![
      RelayerInstruction {
        program_id: crate::id(),
        accounts: vec![
            RelayerAccount::Account { pubkey: incoming_message, is_writable: true },
            RelayerAccount::Account { pubkey: signing_pda, is_writable: false },
            RelayerAccount::Account { pubkey: gateway_root_pda, is_writable: false },
            RelayerAccount::Account { pubkey: event_authority, is_writable: false },
            RelayerAccount::Account { pubkey: GATEWAY_PROGRAM_ID, is_writable: false },
            RelayerAccount::Payer(1000000000),
            RelayerAccount::Account { pubkey: counter_pda, is_writable: true },
            RelayerAccount::Account { pubkey: system_program::ID, is_writable: false },
        ],
        data: vec! [
            RelayerData::Bytes(Vec::from(Execute::DISCRIMINATOR)),
            RelayerData::PayloadRaw,
            RelayerData::Message,
        ]
      }  
    ]))
}