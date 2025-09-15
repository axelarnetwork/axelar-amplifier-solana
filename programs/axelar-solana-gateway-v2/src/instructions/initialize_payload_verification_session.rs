use crate::{signature_verification::VerificationSessionAccount, GatewayConfig};
use anchor_lang::prelude::*;
use axelar_solana_gateway::seed_prefixes::{GATEWAY_SEED, SIGNATURE_VERIFICATION_SEED};

#[derive(Accounts)]
#[instruction(initialize_payload_verification_sesssion: InitializePayloadVerificationSessionInstruction)]
pub struct InitializePayloadVerificationSession<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(
            seeds = [GATEWAY_SEED],
            bump = gateway_root_pda.bump
        )]
    pub gateway_root_pda: Account<'info, GatewayConfig>,
    #[account(
            init,
            payer = payer,
            space = 8 + std::mem::size_of::<VerificationSessionAccount>(),
            seeds = [SIGNATURE_VERIFICATION_SEED, initialize_payload_verification_sesssion.payload_merkle_root.as_ref()],
            bump
        )]
    pub verification_session_account: Account<'info, VerificationSessionAccount>,
    pub system_program: Program<'info, System>,
}

#[derive(Debug, AnchorSerialize, AnchorDeserialize)]
pub struct InitializePayloadVerificationSessionInstruction {
    _padding: u8,
    pub payload_merkle_root: [u8; 32],
}

impl InitializePayloadVerificationSessionInstruction {
    pub fn new(payload_merkle_root: [u8; 32]) -> Self {
        InitializePayloadVerificationSessionInstruction {
            _padding: 0u8,
            payload_merkle_root,
        }
    }
}

pub fn initialize_payload_verification_session_handler(
    ctx: Context<InitializePayloadVerificationSession>,
    _initialize_payload_verification_sesssion: InitializePayloadVerificationSessionInstruction,
) -> Result<()> {
    let bump = ctx.bumps.verification_session_account;
    ctx.accounts.verification_session_account.bump = bump;
    Ok(())
}
