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

pub struct GatewayDiscriminators;

impl GatewayDiscriminators {
    pub const APPROVE_MESSAGE: &'static [u8] = &[0];
    pub const ROTATE_SIGNERS: &'static [u8] = &[1];
    pub const CALL_CONTRACT: &'static [u8] = &[2];
    pub const INITIALIZE_CONFIG: &'static [u8] = &[3];
    pub const INITIALIZE_PAYLOAD_VERIFICATION_SESSION: &'static [u8] = &[4];
    pub const VERIFY_SIGNATURE: &'static [u8] = &[5];
    pub const INITIALIZE_MESSAGE_PAYLOAD: &'static [u8] = &[6];
    pub const WRITE_MESSAGE_PAYLOAD: &'static [u8] = &[7];
    pub const COMMIT_MESSAGE_PAYLOAD: &'static [u8] = &[8];
    pub const CLOSE_MESSAGE_PAYLOAD: &'static [u8] = &[9];
    pub const VALIDATE_MESSAGE: &'static [u8] = &[10];
    pub const TRANSFER_OPERATORSHIP: &'static [u8] = &[11];
}

#[program]
pub mod axelar_solana_gateway_v2 {
    use crate::signature_verification::SigningVerifierSetInfo;

    use super::*;

    pub fn call_contract(
        ctx: Context<CallContract>,
        destination_chain: String,
        destination_contract_address: String,
        payload: Vec<u8>,
    ) -> Result<()> {
        instructions::call_contract_handler(
            ctx,
            destination_chain,
            destination_contract_address,
            payload,
        )
    }

    pub fn initialise_config(
        ctx: Context<InitializeConfigAccounts>,
        params: InitializeConfig,
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
        message: MerkleisedMessage,
        payload_merkle_root: [u8; 32],
    ) -> Result<()> {
        instructions::approve_message_handler(ctx, message, payload_merkle_root)
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
