use crate::{encoding, gmp::*};
use crate::{errors::ItsError, events::TokenMetadataRegistered, state::InterchainTokenService};
use anchor_lang::prelude::*;
use anchor_lang::solana_program::instruction::Instruction;
use anchor_lang::InstructionData;
use anchor_spl::token_interface::Mint;
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

    /// CHECK: checked by the gateway program
    pub gateway_event_authority: UncheckedAccount<'info>,

    /// CHECK: checked by the gas service program
    #[account(mut)]
    pub gas_treasury: UncheckedAccount<'info>,

    /// The GMP gas service program account
    pub gas_service: Program<'info, solana_axelar_gas_service::program::SolanaAxelarGasService>,

    /// CHECK: checked by the gas service program
    pub gas_event_authority: UncheckedAccount<'info>,
}

impl<'info> ToGMPAccounts<'info> for RegisterTokenMetadata<'info> {
    fn to_gmp_accounts(&self) -> GMPAccounts<'info> {
        GMPAccounts {
            payer: self.payer.to_account_info(),
            system_program: self.system_program.to_account_info(),
            gateway_program: self.gateway_program.to_account_info(),
            gateway_root_pda: self.gateway_root_pda.to_account_info(),
            gateway_event_authority: self.gateway_event_authority.to_account_info(),
            call_contract_signing_pda: self.call_contract_signing_pda.to_account_info(),
            its_program: self.program.to_account_info(),
            its_hub_address: self.its_root_pda.its_hub_address.clone(),
            gas_service: self.gas_service.to_account_info(),
            gas_treasury: self.gas_treasury.to_account_info(),
            gas_event_authority: self.gas_event_authority.to_account_info(),
        }
    }
}

pub fn register_token_metadata_handler(
    ctx: Context<RegisterTokenMetadata>,
    gas_value: u64,
) -> Result<()> {
    msg!("Instruction: RegisterTokenMetadata");

    let decimals = ctx.accounts.token_mint.decimals;
    let token_address = ctx.accounts.token_mint.key();

    emit_cpi!(TokenMetadataRegistered {
        token_address,
        decimals,
    });

    let payload = encoding::HubMessage::RegisterTokenMetadata(encoding::RegisterTokenMetadata {
        decimals,
        token_address: token_address.to_bytes().to_vec(),
    });

    let gmp_accounts = ctx.accounts.to_gmp_accounts();

    send_to_hub(gmp_accounts, payload, gas_value)?;

    Ok(())
}

/// Creates a RegisterTokenMetadata instruction
pub fn make_register_token_metadata_instruction(
    payer: Pubkey,
    token_mint: Pubkey,
    gas_value: u64,
) -> (Instruction, crate::accounts::RegisterTokenMetadata) {
    let its_root_pda = InterchainTokenService::find_pda().0;
    let gateway_root_pda = GatewayConfig::find_pda().0;

    let (call_contract_signing_pda, _) =
        solana_axelar_gateway::CallContractSigner::find_pda(&crate::ID);

    let (gateway_event_authority, _) = solana_axelar_gateway::EVENT_AUTHORITY_AND_BUMP;

    let (gas_treasury, _) = Pubkey::find_program_address(
        &[solana_axelar_gas_service::state::Treasury::SEED_PREFIX],
        &solana_axelar_gas_service::ID,
    );

    let (gas_event_authority, _) = solana_axelar_gas_service::EVENT_AUTHORITY_AND_BUMP;

    let (event_authority, _) = crate::EVENT_AUTHORITY_AND_BUMP;

    let accounts = crate::accounts::RegisterTokenMetadata {
        payer,
        token_mint,
        gateway_root_pda,
        gateway_program: solana_axelar_gateway::ID,
        system_program: anchor_lang::system_program::ID,
        its_root_pda,
        call_contract_signing_pda,
        gateway_event_authority,
        gas_treasury,
        gas_service: solana_axelar_gas_service::ID,
        gas_event_authority,
        event_authority,
        program: crate::ID,
    };

    (
        Instruction {
            program_id: crate::ID,
            accounts: accounts.to_account_metas(None),
            data: crate::instruction::RegisterTokenMetadata { gas_value }.data(),
        },
        accounts,
    )
}
