use crate::gmp::{GMPAccounts, ToGMPAccounts};
use crate::instructions::process_outbound;
use crate::{
    errors::ITSError, events::TokenMetadataRegistered, state::InterchainTokenService,
    ITS_HUB_CHAIN_NAME,
};
use anchor_lang::prelude::*;
use anchor_spl::token_2022::spl_token_2022::{
    extension::StateWithExtensions, state::Mint as SplMint,
};
use anchor_spl::token_interface::Mint;
use axelar_solana_gas_service_v2::state::Treasury;
use axelar_solana_gateway_v2::{seed_prefixes::CALL_CONTRACT_SIGNING_SEED, GatewayConfig};
use interchain_token_transfer_gmp::{
    GMPPayload, RegisterTokenMetadata as RegisterTokenMetadataPayload,
};

#[derive(Accounts)]
#[event_cpi]
#[instruction(gas_value: u64, signing_pda_bump: u8)]
pub struct RegisterTokenMetadata<'info> {
    /// The account which is paying for the transaction
    #[account(mut)]
    pub payer: Signer<'info>,

    /// The mint account (token address) to register metadata for
    pub token_mint: InterfaceAccount<'info, Mint>,

    // GMP accounts
    /// The GMP gateway root account
    #[account(
        seeds = [
            axelar_solana_gateway_v2::seed_prefixes::GATEWAY_SEED
        ],
        seeds::program = axelar_solana_gateway_v2::ID,
        bump = gateway_root_pda.load()?.bump,
    )]
    pub gateway_root_pda: AccountLoader<'info, GatewayConfig>,

    /// The GMP gateway program account
    #[account(address = axelar_solana_gateway_v2::ID)]
    pub axelar_gateway_program: AccountInfo<'info>,

    /// The GMP gas treasury account
    #[account(
        mut,
        seeds = [Treasury::SEED_PREFIX],
        seeds::program = axelar_solana_gas_service_v2::ID,
        bump = gas_treasury.load()?.bump,
    )]
    pub gas_treasury: AccountLoader<'info, Treasury>,

    /// The GMP gas service program account
    #[account(address = axelar_solana_gas_service_v2::ID)]
    pub gas_service: AccountInfo<'info>,

    /// The system program account
    pub system_program: Program<'info, System>,

    /// The ITS root account
    #[account(
        seeds = [InterchainTokenService::SEED_PREFIX],
        bump = its_root_pda.bump,
        constraint = !its_root_pda.paused @ ITSError::Paused
    )]
    pub its_root_pda: Account<'info, InterchainTokenService>,

    /// The GMP call contract signing account
    #[account(
        seeds = [CALL_CONTRACT_SIGNING_SEED],
        bump = signing_pda_bump,
        seeds::program = crate::ID
    )]
    pub call_contract_signing_pda: Signer<'info>,

    /// The ITS program account (this program)
    #[account(address = crate::ID)]
    pub its_program: AccountInfo<'info>,

    /// Event authority - derived from gateway program
    #[account(
        seeds = [b"__event_authority"],
        bump,
        seeds::program = axelar_gateway_program.key()
    )]
    pub gateway_event_authority: SystemAccount<'info>,

    /// Event authority for gas service - derived from gas service program
    #[account(
        seeds = [b"__event_authority"],
        bump,
        seeds::program = gas_service.key()
    )]
    pub gas_event_authority: SystemAccount<'info>,
}

impl<'info> ToGMPAccounts<'info> for RegisterTokenMetadata<'info> {
    fn to_gmp_accounts(&self) -> GMPAccounts<'info> {
        GMPAccounts {
            payer: self.payer.to_account_info(),
            gateway_root_pda: self.gateway_root_pda.to_account_info(),
            axelar_gateway_program: self.axelar_gateway_program.clone(),
            gas_treasury: self.gas_treasury.to_account_info(),
            gas_service: self.gas_service.clone(),
            system_program: self.system_program.to_account_info(),
            its_root_pda: self.its_root_pda.clone(),
            call_contract_signing_pda: self.call_contract_signing_pda.to_account_info(),
            its_program: self.its_program.clone(),
            gateway_event_authority: self.gateway_event_authority.to_account_info(),
            gas_event_authority: self.gas_event_authority.to_account_info(),
        }
    }
}

pub fn register_token_metadata_handler(
    ctx: Context<RegisterTokenMetadata>,
    gas_value: u64,
    signing_pda_bump: u8,
) -> Result<()> {
    msg!("Instruction: RegisterTokenMetadata");

    let token_mint_account = ctx.accounts.token_mint.to_account_info();
    let mint_data = token_mint_account.try_borrow_data()?;
    let mint = StateWithExtensions::<SplMint>::unpack(&mint_data)?;
    let decimals = mint.base.decimals;

    // Create the register token metadata payload
    let inner_payload = GMPPayload::RegisterTokenMetadata(RegisterTokenMetadataPayload {
        selector: RegisterTokenMetadataPayload::MESSAGE_TYPE_ID
            .try_into()
            .map_err(|_err| ProgramError::ArithmeticOverflow)?,
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
        ITS_HUB_CHAIN_NAME.to_string(),
        gas_value,
        signing_pda_bump,
        inner_payload,
    )?;

    Ok(())
}
