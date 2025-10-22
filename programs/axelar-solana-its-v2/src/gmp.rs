use crate::state::InterchainTokenService;
use anchor_lang::prelude::*;

/// Common GMP accounts needed for outbound operations
#[derive(Clone)]
pub struct GMPAccounts<'info> {
    pub payer: AccountInfo<'info>,
    pub gateway_root_pda: AccountInfo<'info>,
    pub axelar_gateway_program: AccountInfo<'info>,
    pub gas_treasury: AccountInfo<'info>,
    pub gas_service: AccountInfo<'info>,
    pub system_program: AccountInfo<'info>,
    pub its_root_pda: Account<'info, InterchainTokenService>,
    pub call_contract_signing_pda: AccountInfo<'info>,
    pub its_program: AccountInfo<'info>,
    pub gateway_event_authority: AccountInfo<'info>,
    pub gas_event_authority: AccountInfo<'info>,
}

pub trait ToGMPAccounts<'info> {
    fn to_gmp_accounts(&self) -> GMPAccounts<'info>;
}
