use anchor_lang::prelude::*;
use anchor_lang::solana_program;
use solana_axelar_gas_service::cpi::{accounts::PayGas, pay_gas};
use solana_axelar_gateway::seed_prefixes::CALL_CONTRACT_SIGNING_SEED;

use crate::ItsError;
use crate::ITS_HUB_CHAIN_NAME;

/// Common GMP accounts needed for outbound operations
#[derive(Clone)]
pub struct GMPAccounts<'info> {
    pub payer: AccountInfo<'info>,
    pub system_program: AccountInfo<'info>,
    pub gateway_program: AccountInfo<'info>,
    pub gateway_root_pda: AccountInfo<'info>,
    pub gateway_event_authority: AccountInfo<'info>,
    pub call_contract_signing_pda: AccountInfo<'info>,
    pub its_program: AccountInfo<'info>,
    pub its_hub_address: String,
    pub gas_service: AccountInfo<'info>,
    pub gas_treasury: AccountInfo<'info>,
    pub gas_event_authority: AccountInfo<'info>,
}

pub trait ToGMPAccounts<'info> {
    fn to_gmp_accounts(&self) -> GMPAccounts<'info>;
}

//
// Outbound GMP payloads
//

pub fn send_to_hub_wrap(
    gmp_accounts: GMPAccounts,
    message: crate::encoding::Message,
    destination_chain: String,
    gas_value: u64,
) -> Result<()> {
    use crate::encoding::HubMessage;

    let payload = HubMessage::SendToHub {
        destination_chain,
        message,
    };

    send_to_hub(gmp_accounts, payload, gas_value)
}

pub fn send_to_hub(
    gmp_accounts: GMPAccounts,
    payload: crate::encoding::HubMessage,
    gas_value: u64,
) -> Result<()> {
    if matches!(payload, crate::encoding::HubMessage::ReceiveFromHub { .. }) {
        return Err(ItsError::InvalidInstructionData.into());
    }

    let payload = payload
        .try_to_vec()
        // TODO better error
        .map_err(|_| ItsError::InvalidArgument)?;
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
    let (expected_signing_pda, signing_pda_bump) =
        Pubkey::find_program_address(&[CALL_CONTRACT_SIGNING_SEED], &crate::ID);

    if expected_signing_pda != *gmp_accounts.call_contract_signing_pda.key {
        return Err(ItsError::InvalidAccountData.into());
    }

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
        ITS_HUB_CHAIN_NAME.to_owned(),
        destination_address,
        payload,
        signing_pda_bump,
    )?;

    Ok(())
}
