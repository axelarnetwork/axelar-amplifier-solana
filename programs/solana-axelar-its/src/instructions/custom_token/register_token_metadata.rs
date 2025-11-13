use crate::gmp::*;
use crate::{
    errors::ItsError, events::TokenMetadataRegistered, state::InterchainTokenService,
    ITS_HUB_CHAIN_NAME,
};
use anchor_lang::prelude::*;
use anchor_spl::token_interface::Mint;
use interchain_token_transfer_gmp::{
    GMPPayload, RegisterTokenMetadata as RegisterTokenMetadataPayload,
};
use solana_axelar_gateway::{program::SolanaAxelarGateway, GatewayConfig};

#[derive(Accounts)]
#[event_cpi]
#[instruction(gas_value: u64)]
pub struct RegisterTokenMetadata<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    pub token_mint: InterfaceAccount<'info, Mint>,

    // GMP accounts
    #[account(
        seeds = [
            solana_axelar_gateway::seed_prefixes::GATEWAY_SEED
        ],
        seeds::program = solana_axelar_gateway::ID,
        bump = gateway_root_pda.load()?.bump,
    )]
    pub gateway_root_pda: AccountLoader<'info, GatewayConfig>,

    /// The GMP gateway program account
    pub gateway_program: Program<'info, SolanaAxelarGateway>,

    pub system_program: Program<'info, System>,

    #[account(
        seeds = [InterchainTokenService::SEED_PREFIX],
        bump = its_root_pda.bump,
        constraint = !its_root_pda.paused @ ItsError::Paused
    )]
    pub its_root_pda: Account<'info, InterchainTokenService>,

    /// CHECK: validated in gateway
    pub call_contract_signing_pda: UncheckedAccount<'info>,

    // Event authority accounts
    #[account(
        seeds = [b"__event_authority"],
        bump,
        seeds::program = solana_axelar_gateway::ID,
    )]
    pub gateway_event_authority: AccountInfo<'info>,

    pub gas_service_accounts: GasServiceAccounts<'info>,
}

impl<'info> ToGMPAccounts<'info> for RegisterTokenMetadata<'info> {
    fn to_gmp_accounts(&self) -> GMPAccounts<'info> {
        GMPAccounts {
            payer: self.payer.to_account_info(),
            gateway_root_pda: self.gateway_root_pda.to_account_info(),
            gateway_program: self.gateway_program.to_account_info(),
            gas_treasury: self.gas_service_accounts.gas_treasury.to_account_info(),
            gas_service: self.gas_service_accounts.gas_service.to_account_info(),
            system_program: self.system_program.to_account_info(),
            its_hub_address: self.its_root_pda.its_hub_address.clone(),
            call_contract_signing_pda: self.call_contract_signing_pda.to_account_info(),
            its_program: self.program.to_account_info(),
            gateway_event_authority: self.gateway_event_authority.to_account_info(),
            gas_event_authority: self
                .gas_service_accounts
                .gas_event_authority
                .to_account_info(),
        }
    }
}

pub fn register_token_metadata_handler(
    ctx: Context<RegisterTokenMetadata>,
    gas_value: u64,
) -> Result<()> {
    msg!("Instruction: RegisterTokenMetadata");

    let decimals = ctx.accounts.token_mint.decimals;

    // Create the register token metadata payload
    let inner_payload = GMPPayload::RegisterTokenMetadata(RegisterTokenMetadataPayload {
        selector: RegisterTokenMetadataPayload::MESSAGE_TYPE_ID
            .try_into()
            .map_err(|_err| ItsError::ArithmeticOverflow)?,
        token_address: ctx.accounts.token_mint.key().to_bytes().into(),
        decimals,
    });

    emit_cpi!(TokenMetadataRegistered {
        token_address: ctx.accounts.token_mint.key(),
        decimals,
    });

    let gmp_accounts = ctx.accounts.to_gmp_accounts();
    process_outbound(
        gmp_accounts,
        ITS_HUB_CHAIN_NAME.to_owned(),
        gas_value,
        inner_payload,
        false, // don't wrap inner payload
    )?;

    Ok(())
}
