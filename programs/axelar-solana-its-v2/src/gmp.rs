use crate::state::InterchainTokenService;
use anchor_lang::prelude::*;

/// Common GMP accounts needed for outbound operations
#[derive(Clone)]
pub struct GMPAccounts<'info> {
    pub payer: AccountInfo<'info>,
    pub gateway_root_pda: AccountInfo<'info>,
    pub gateway_program: AccountInfo<'info>,
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

#[derive(Accounts)]
pub struct GasServiceAccounts<'info> {
    /// The GMP gas treasury account
    #[account(
        mut,
        seeds = [axelar_solana_gas_service_v2::state::Treasury::SEED_PREFIX],
        seeds::program = axelar_solana_gas_service_v2::ID,
        bump = gas_treasury.load()?.bump,
    )]
    pub gas_treasury: AccountLoader<'info, axelar_solana_gas_service_v2::state::Treasury>,

    /// The GMP gas service program account
    pub gas_service:
        Program<'info, axelar_solana_gas_service_v2::program::AxelarSolanaGasServiceV2>,

    /// Event authority for gas service
    #[account(
        seeds = [b"__event_authority"],
        bump,
        seeds::program = gas_service.key()
    )]
    pub gas_event_authority: AccountInfo<'info>,
}
