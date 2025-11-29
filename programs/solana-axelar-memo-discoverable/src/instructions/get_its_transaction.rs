use crate::{Counter, Payload};
use anchor_lang::{prelude::*, system_program};
use interchain_token_transfer_gmp::GMPPayload;
use relayer_discovery::structs::{RelayerAccount, RelayerTransaction};
use solana_axelar_its::{
    instructions::{decode_interchain_transfer_payload, insterchain_transfer_transaction},
    InterchainTokenService, TokenManager,
};
use solana_axelar_std::Message;

#[derive(Accounts)]
#[instruction(
    message: Message,
    payload: Vec<u8>,
)]
pub struct GetItsTransaction<'info> {
    #[account(
        seeds = [
            TokenManager::SEED_PREFIX,
            InterchainTokenService::find_pda().0.key().as_ref(),
            GMPPayload::decode(&payload).unwrap().token_id().unwrap().as_ref(),
        ],
        bump = token_manager_pda.bump,
        seeds::program = solana_axelar_its::ID,
    )]
    pub token_manager_pda: Account<'info, TokenManager>,
}

/// This should return a `RelayerTransaction` that will convert to an `Execute` instruction properly, for a given `payload` and `command_id`. No accounts are needed to find this information.
///
pub fn get_its_transaction_handler(
    ctx: Context<GetItsTransaction>,
    message: Message,
    payload: Vec<u8>,
) -> Result<RelayerTransaction> {
    let (transfer, source_chain) = decode_interchain_transfer_payload(payload)?;

    let payload = Payload::deserialize(&mut transfer.data.clone().iter().as_slice())?;
    let counter_pda = Counter::get_pda(payload.storage_id).0;

    insterchain_transfer_transaction(
        message,
        transfer,
        source_chain,
        Some(ctx.accounts.token_manager_pda.clone()),
        None,
        Some(vec![
            RelayerAccount::Payer(1_010_000),
            RelayerAccount::Account {
                pubkey: counter_pda,
                is_writable: true,
            },
            RelayerAccount::Account {
                pubkey: system_program::ID,
                is_writable: false,
            },
        ]),
    )
}
