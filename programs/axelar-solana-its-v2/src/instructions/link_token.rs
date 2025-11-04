use crate::{
    errors::ItsError,
    events::{InterchainTokenIdClaimed, LinkTokenStarted},
    gmp::*,
    instructions::process_outbound,
    program::AxelarSolanaItsV2,
    state::{
        token_manager::{TokenManager, Type},
        InterchainTokenService,
    },
    utils::{interchain_token_id_internal, linked_token_deployer_salt},
};
use anchor_lang::prelude::*;
use axelar_solana_gateway_v2::{
    program::AxelarSolanaGatewayV2, seed_prefixes::CALL_CONTRACT_SIGNING_SEED, GatewayConfig,
};
use interchain_token_transfer_gmp::{GMPPayload, LinkToken as LinkTokenPayload};

#[derive(Accounts)]
#[instruction(
    salt: [u8; 32],
    destination_chain: String,
    destination_token_address: Vec<u8>,
    token_manager_type: Type,
    link_params: Vec<u8>,
    gas_value: u64,
    signing_pda_bump: u8,
)]
#[event_cpi]
pub struct LinkToken<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    pub deployer: Signer<'info>,

    pub its_program: Program<'info, AxelarSolanaItsV2>,

    #[account(
        seeds = [InterchainTokenService::SEED_PREFIX],
        bump = its_root_pda.bump,
        constraint = !its_root_pda.paused @ ItsError::Paused,
        constraint = its_root_pda.chain_name != destination_chain @ ItsError::InvalidDestinationChain,
        constraint = its_root_pda.is_trusted_chain_or_hub(&destination_chain) @ ItsError::UntrustedDestinationChain,
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
            axelar_solana_gateway_v2::seed_prefixes::GATEWAY_SEED
        ],
        seeds::program = axelar_solana_gateway_v2::ID,
        bump = gateway_root_pda.load()?.bump,
    )]
    pub gateway_root_pda: AccountLoader<'info, GatewayConfig>,

    pub gateway_program: Program<'info, AxelarSolanaGatewayV2>,

    pub system_program: Program<'info, System>,

    #[account(
        seeds = [CALL_CONTRACT_SIGNING_SEED],
        bump = signing_pda_bump,
        seeds::program = crate::ID
    )]
    pub call_contract_signing_pda: AccountInfo<'info>,

    // Event authority accounts
    #[account(
        seeds = [b"__event_authority"],
        bump,
        seeds::program = axelar_solana_gateway_v2::ID,
    )]
    pub gateway_event_authority: SystemAccount<'info>,

    pub gas_service_accounts: GasServiceAccounts<'info>,
}

impl<'info> LinkToken<'info> {
    pub fn to_gmp_accounts(&self) -> GMPAccounts<'info> {
        GMPAccounts {
            payer: self.payer.to_account_info(),
            gateway_root_pda: self.gateway_root_pda.to_account_info(),
            gateway_program: self.gateway_program.to_account_info(),
            system_program: self.system_program.to_account_info(),
            its_root_pda: self.its_root_pda.clone(),
            call_contract_signing_pda: self.call_contract_signing_pda.to_account_info(),
            its_program: self.its_program.to_account_info(),
            gateway_event_authority: self.gateway_event_authority.to_account_info(),
            // Gas Service
            gas_treasury: self.gas_service_accounts.gas_treasury.to_account_info(),
            gas_service: self.gas_service_accounts.gas_service.to_account_info(),
            gas_event_authority: self
                .gas_service_accounts
                .gas_event_authority
                .to_account_info(),
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
    signing_pda_bump: u8,
) -> Result<[u8; 32]> {
    msg!("Instruction: LinkToken");

    if token_manager_type == Type::NativeInterchainToken {
        return err!(ItsError::InvalidInstructionData);
    }

    // Derive the token ID using the same logic as the existing implementation
    let deploy_salt = linked_token_deployer_salt(&ctx.accounts.deployer.key(), &salt);
    let token_id = interchain_token_id_internal(&deploy_salt);

    // Emit InterchainTokenIdClaimed event
    emit_cpi!(InterchainTokenIdClaimed {
        token_id,
        deployer: ctx.accounts.deployer.key(),
        salt: deploy_salt,
    });

    // Emit LinkTokenStarted event
    emit_cpi!(LinkTokenStarted {
        token_id,
        destination_chain: destination_chain.clone(),
        source_token_address: ctx.accounts.token_manager_pda.token_address,
        destination_token_address: destination_token_address.clone(),
        token_manager_type: token_manager_type.into(),
        params: link_params.clone(),
    });

    // Create the GMP payload for linking the token
    let message = GMPPayload::LinkToken(LinkTokenPayload {
        selector: LinkTokenPayload::MESSAGE_TYPE_ID
            .try_into()
            .map_err(|_err| ProgramError::ArithmeticOverflow)?,
        token_id: token_id.into(),
        token_manager_type: token_manager_type.into(),
        source_token_address: ctx
            .accounts
            .token_manager_pda
            .token_address
            .to_bytes()
            .into(),
        destination_token_address: destination_token_address.into(),
        link_params: link_params.into(),
    });

    let gmp_accounts = ctx.accounts.to_gmp_accounts();

    // Process the outbound GMP message
    process_outbound(
        gmp_accounts,
        destination_chain,
        gas_value,
        signing_pda_bump,
        message,
    )?;

    Ok(token_id)
}
