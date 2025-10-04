use anchor_lang::prelude::*;

pub mod instructions;
pub use instructions::*;

pub mod state;
pub use state::*;

pub mod events;
pub use events::*;

pub mod types;
pub use types::*;

pub mod errors;
pub use errors::*;

declare_id!("7ZhLjSZJ7zWATu6PtYGgfU2V6B6EYSQTX3hDo4KtWuwZ");

#[program]
pub mod axelar_solana_gateway_v2 {
    use super::*;
    use crate::signature_verification::SigningVerifierSetInfo;

    pub fn call_contract(
        ctx: Context<CallContract>,
        destination_chain: String,
        destination_contract_address: String,
        payload: Vec<u8>,
        signing_pda_bump: u8,
    ) -> Result<()> {
        instructions::call_contract_handler(
            ctx,
            destination_chain,
            destination_contract_address,
            payload,
            signing_pda_bump,
        )
    }

    pub fn initialize_config(
        ctx: Context<InitializeConfigAccounts>,
        params: InitializeConfigParams,
    ) -> Result<()> {
        instructions::initialize_config_handler(ctx, params)
    }

    pub fn initialize_payload_verification_session(
        ctx: Context<InitializePayloadVerificationSession>,
        merkle_root: [u8; 32],
    ) -> Result<()> {
        instructions::initialize_payload_verification_session_handler(ctx, merkle_root)
    }

    pub fn verify_signature(
        ctx: Context<VerifySignature>,
        payload_merkle_root: [u8; 32],
        verifier_info: SigningVerifierSetInfo,
    ) -> Result<()> {
        instructions::verify_signature_handler(ctx, payload_merkle_root, verifier_info)
    }

    pub fn approve_message(
        ctx: Context<ApproveMessage>,
        merkleised_message: MerkleisedMessage,
        payload_merkle_root: [u8; 32],
    ) -> Result<()> {
        instructions::approve_message_handler(ctx, merkleised_message, payload_merkle_root)
    }

    pub fn validate_message(ctx: Context<ValidateMessage>, message: Message) -> Result<()> {
        instructions::validate_message_handler(ctx, message)
    }

    pub fn rotate_signers(
        ctx: Context<RotateSigners>,
        new_verifier_set_merkle_root: [u8; 32],
    ) -> Result<()> {
        instructions::rotate_signers_handler(ctx, new_verifier_set_merkle_root)
    }

    pub fn transfer_operatorship(ctx: Context<TransferOperatorship>) -> Result<()> {
        instructions::transfer_operatorship_handler(ctx)
    }
}
