use crate::{
    encoding,
    errors::ItsError,
    events::LinkTokenStarted,
    gmp::*,
    state::{
        token_manager::{TokenManager, Type},
        InterchainTokenService,
    },
    utils::{interchain_token_id_internal, linked_token_deployer_salt},
};
use anchor_lang::prelude::*;
use solana_axelar_gateway::{program::SolanaAxelarGateway, GatewayConfig};

#[derive(Accounts)]
#[instruction(
    salt: [u8; 32],
    destination_chain: String,
    destination_token_address: Vec<u8>,
    token_manager_type: Type,
    link_params: Vec<u8>,
    gas_value: u64,
)]
#[event_cpi]
pub struct LinkToken<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    pub deployer: Signer<'info>,

    #[account(
        seeds = [InterchainTokenService::SEED_PREFIX],
        bump = its_root_pda.bump,
        constraint = !its_root_pda.paused @ ItsError::Paused,
        constraint = its_root_pda.chain_name != destination_chain @ ItsError::InvalidDestinationChain,
        constraint = its_root_pda.is_trusted_chain(&destination_chain) @ ItsError::UntrustedDestinationChain,
    )]
    pub its_root_pda: Account<'info, InterchainTokenService>,

    #[account(
        seeds = [
            TokenManager::SEED_PREFIX,
            its_root_pda.key().as_ref(),
            &interchain_token_id_internal(&linked_token_deployer_salt(&deployer.key(), &salt))
        ],
        bump = token_manager_pda.bump,
    )]
    pub token_manager_pda: Account<'info, TokenManager>,

    // GMP Accounts
    #[account(
        seeds = [
            solana_axelar_gateway::seed_prefixes::GATEWAY_SEED
        ],
        seeds::program = solana_axelar_gateway::ID,
        bump = gateway_root_pda.load()?.bump,
    )]
    pub gateway_root_pda: AccountLoader<'info, GatewayConfig>,

    pub gateway_program: Program<'info, SolanaAxelarGateway>,

    pub system_program: Program<'info, System>,

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

impl<'info> LinkToken<'info> {
    pub fn to_gmp_accounts(&self) -> GMPAccounts<'info> {
        GMPAccounts {
            payer: self.payer.to_account_info(),
            gateway_root_pda: self.gateway_root_pda.to_account_info(),
            gateway_program: self.gateway_program.to_account_info(),
            gateway_event_authority: self.gateway_event_authority.to_account_info(),
            system_program: self.system_program.to_account_info(),
            its_hub_address: self.its_root_pda.its_hub_address.clone(),
            call_contract_signing_pda: self.call_contract_signing_pda.to_account_info(),
            its_program: self.program.to_account_info(),
            // Gas Service
            gas_treasury: self.gas_treasury.to_account_info(),
            gas_service: self.gas_service.to_account_info(),
            gas_event_authority: self.gas_event_authority.to_account_info(),
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn link_token_handler(
    ctx: Context<LinkToken>,
    salt: [u8; 32],
    destination_chain: String,
    destination_token_address: Vec<u8>,
    token_manager_type: Type,
    link_params: Vec<u8>,
    gas_value: u64,
) -> Result<[u8; 32]> {
    msg!("Instruction: LinkToken");

    if token_manager_type == Type::NativeInterchainToken {
        return err!(ItsError::InvalidInstructionData);
    }
    if destination_token_address.is_empty() {
        return err!(ItsError::InvalidDestinationAddress);
    }

    // Derive the token ID using the same logic as the existing implementation
    let deploy_salt = linked_token_deployer_salt(&ctx.accounts.deployer.key(), &salt);
    let token_id = interchain_token_id_internal(&deploy_salt);

    // Emit LinkTokenStarted event
    emit_cpi!(LinkTokenStarted {
        token_id,
        destination_chain: destination_chain.clone(),
        source_token_address: ctx.accounts.token_manager_pda.token_address,
        destination_token_address: destination_token_address.clone(),
        token_manager_type: token_manager_type.into(),
        params: if link_params.is_empty() {
            None
        } else {
            Some(link_params.clone())
        },
    });

    let payload = encoding::Message::LinkToken(encoding::LinkToken {
        token_id,
        token_manager_type: token_manager_type.into(),
        source_token_address: ctx
            .accounts
            .token_manager_pda
            .token_address
            .to_bytes()
            .to_vec(),
        destination_token_address,
        params: if link_params.is_empty() {
            None
        } else {
            Some(link_params)
        },
    });

    let gmp_accounts = ctx.accounts.to_gmp_accounts();

    // Process the outbound GMP message
    send_to_hub_wrap(gmp_accounts, payload, destination_chain, gas_value)?;

    Ok(token_id)
}
