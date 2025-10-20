use crate::{
    errors::ITSError,
    events::{InterchainTokenIdClaimed, LinkTokenStarted},
    instructions::{process_outbound, GMPAccounts},
    state::{
        token_manager::{TokenManager, Type},
        InterchainTokenService,
    },
    utils::{interchain_token_id_internal, linked_token_deployer_salt},
};
use anchor_lang::prelude::*;
use axelar_solana_gas_service_v2::state::Treasury;
use axelar_solana_gateway_v2::{seed_prefixes::CALL_CONTRACT_SIGNING_SEED, GatewayConfig};
use interchain_token_transfer_gmp::{GMPPayload, LinkToken as LinkTokenPayload};

#[derive(Accounts)]
#[instruction(
    salt: [u8; 32],
    destination_chain: String,
    destination_token_address: Vec<u8>,
    token_manager_type: Type,
    link_params: Vec<u8>,
    gas_value: u64,
    signing_pda_bump: u8
)]
#[event_cpi]
pub struct LinkToken<'info> {
    /// Payer for the transaction fees (must be signer and writable)
    #[account(mut)]
    pub payer: Signer<'info>,

    /// The deployer who originally deployed the token (must be signer)
    pub deployer: Signer<'info>,

    /// The token manager account associated with the canonical interchain token
    #[account(
        seeds = [
            crate::seed_prefixes::TOKEN_MANAGER_SEED,
            its_root_pda.key().as_ref(),
            &interchain_token_id_internal(&linked_token_deployer_salt(&deployer.key(), &salt))
        ],
        seeds::program = crate::ID,
        bump = token_manager_pda.bump,
    )]
    pub token_manager_pda: Account<'info, TokenManager>,

    // GMP Accounts
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

impl<'info> LinkToken<'info> {
    /// Convert the accounts to GmpAccounts format expected by the GMP processor
    pub fn to_gmp_accounts(&self) -> GMPAccounts<'info> {
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

pub fn link_token_handler(
    ctx: Context<LinkToken>,
    salt: [u8; 32],
    destination_chain: String,
    destination_token_address: Vec<u8>,
    token_manager_type: Type,
    link_params: Vec<u8>,
    gas_value: u64,
    signing_pda_bump: u8,
) -> Result<()> {
    msg!("Instruction: LinkToken");

    // Validate that destination chain is different from current chain
    if destination_chain == ctx.accounts.its_root_pda.chain_name {
        msg!("Cannot link to another token on the same chain");
        return err!(ITSError::InvalidInstructionData);
    }

    if token_manager_type == Type::NativeInterchainToken {
        return err!(ITSError::InvalidInstructionData);
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

    // Set return data to the token_id
    anchor_lang::solana_program::program::set_return_data(&token_id);

    Ok(())
}
