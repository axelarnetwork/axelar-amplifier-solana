use anchor_lang::prelude::*;
use anchor_lang::solana_program;
use interchain_token_transfer_gmp::{GMPPayload, SendToHub};
use solana_axelar_gas_service::cpi::{accounts::PayGas, pay_gas};
use solana_axelar_gateway::seed_prefixes::CALL_CONTRACT_SIGNING_SEED;

use crate::ItsError;
use crate::ITS_HUB_CHAIN_NAME;

/// Common GMP accounts needed for outbound operations
#[derive(Clone)]
pub struct GMPAccounts<'info> {
    pub payer: AccountInfo<'info>,
    pub gateway_root_pda: AccountInfo<'info>,
    pub gateway_program: AccountInfo<'info>,
    pub gas_treasury: AccountInfo<'info>,
    pub gas_service: AccountInfo<'info>,
    pub system_program: AccountInfo<'info>,
    pub its_hub_address: String,
    pub call_contract_signing_pda: AccountInfo<'info>,
    pub its_program: AccountInfo<'info>,
    pub gateway_event_authority: AccountInfo<'info>,
    pub gas_event_authority: AccountInfo<'info>,
}

pub trait ToGMPAccounts<'info> {
    fn to_gmp_accounts(&self) -> GMPAccounts<'info>;
}

#[derive(Accounts)]
pub struct GasServiceAccounts<'info> {
    /// The GMP gas treasury account
    #[account(
        mut,
        seeds = [solana_axelar_gas_service::state::Treasury::SEED_PREFIX],
        seeds::program = solana_axelar_gas_service::ID,
        bump = gas_treasury.load()?.bump,
    )]
    pub gas_treasury: AccountLoader<'info, solana_axelar_gas_service::state::Treasury>,

    /// The GMP gas service program account
    pub gas_service: Program<'info, solana_axelar_gas_service::program::SolanaAxelarGasService>,

    /// Event authority for gas service
    #[account(
        seeds = [b"__event_authority"],
        bump,
        seeds::program = gas_service.key()
    )]
    pub gas_event_authority: AccountInfo<'info>,
}

//
// Outbound GMP payloads
//

pub fn process_outbound(
    gmp_accounts: GMPAccounts,
    destination_chain: String,
    gas_value: u64,
    inner_payload: GMPPayload,
    should_send_to_hub: bool,
) -> Result<()> {
    // Wrap the inner payload if we need to send to hub
    let payload = if should_send_to_hub {
        GMPPayload::SendToHub(SendToHub {
            selector: SendToHub::MESSAGE_TYPE_ID
                .try_into()
                .map_err(|_err| ItsError::ArithmeticOverflow)?,
            destination_chain: destination_chain.clone(),
            payload: inner_payload.encode().into(),
        })
        .encode()
    } else {
        inner_payload.encode()
    };

    let payload_hash = solana_program::keccak::hash(&payload).to_bytes();
    let destination_address = gmp_accounts.its_hub_address;
    let refund_address = gmp_accounts.payer.key();

    if gas_value > 0 {
        let cpi_accounts = PayGas {
            sender: gmp_accounts.payer,
            treasury: gmp_accounts.gas_treasury,
            system_program: gmp_accounts.system_program,
            event_authority: gmp_accounts.gas_event_authority,
            program: gmp_accounts.gas_service.clone(),
        };

        let cpi_ctx = CpiContext::new(gmp_accounts.gas_service, cpi_accounts);

        pay_gas(
            cpi_ctx,
            ITS_HUB_CHAIN_NAME.to_owned(),
            destination_address.clone(),
            payload_hash,
            gas_value,
            refund_address,
        )?;
    }

    // Call contract instruction

    // NOTE: this could be calculated at compile time
    let (_, signing_pda_bump) =
        Pubkey::find_program_address(&[CALL_CONTRACT_SIGNING_SEED], &crate::ID);

    let signer_seeds: &[&[&[u8]]] = &[&[CALL_CONTRACT_SIGNING_SEED, &[signing_pda_bump]]];

    let cpi_accounts = solana_axelar_gateway::cpi::accounts::CallContract {
        caller: gmp_accounts.its_program.to_account_info(),
        signing_pda: Some(gmp_accounts.call_contract_signing_pda.to_account_info()),
        gateway_root_pda: gmp_accounts.gateway_root_pda.to_account_info(),
        // For event_cpi
        event_authority: gmp_accounts.gateway_event_authority.to_account_info(),
        program: gmp_accounts.gateway_program.to_account_info(),
    };

    let cpi_ctx = CpiContext::new_with_signer(
        gmp_accounts.gateway_program.to_account_info(),
        cpi_accounts,
        signer_seeds,
    );

    solana_axelar_gateway::cpi::call_contract(
        cpi_ctx,
        destination_chain,
        destination_address,
        payload,
        signing_pda_bump,
    )?;

    Ok(())
}
