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

    pub fn call_contract(
        ctx: Context<CallContract>,
        call_contract_instruction: CallContractInstruction,
    ) -> Result<()> {
        instructions::call_contract_handler(ctx, call_contract_instruction)
    }

    pub fn initialise_config(
        ctx: Context<InitializeConfigAccounts>,
        params: InitializeConfigInstruction,
    ) -> Result<()> {
        instructions::initialize_config_handler(ctx, params)
    }

    pub fn initialize_payload_verification_session(
        ctx: Context<InitializePayloadVerificationSession>,
        initialize_payload_verification_sesssion: InitializePayloadVerificationSessionInstruction,
    ) -> Result<()> {
        instructions::initialize_payload_verification_session_handler(
            ctx,
            initialize_payload_verification_sesssion,
        )
    }

    pub fn verify_signature(
        ctx: Context<VerifySignature>,
        verify_signature_instruction: VerifySignatureInstruction,
    ) -> Result<()> {
        instructions::verify_signature_handler(ctx, verify_signature_instruction)
    }

    pub fn approve_message(
        ctx: Context<ApproveMessage>,
        approve_message_instruction: ApproveMessageInstruction,
    ) -> Result<()> {
        instructions::approve_message_handler(ctx, approve_message_instruction)
    }

    pub fn validate_message(
        ctx: Context<ValidateMessage>,
        validate_message_instruction: ValidateMessageInstruction,
    ) -> Result<()> {
        instructions::validate_message_handler(ctx, validate_message_instruction)
    }

    pub fn rotate_signers(
        ctx: Context<RotateSigners>,
        rotate_signers_instruction: RotateSignersInstruction,
    ) -> Result<()> {
        instructions::rotate_signers_handler(ctx, rotate_signers_instruction)
    }

    pub fn transfer_operatorship(ctx: Context<TransferOperatorship>) -> Result<()> {
        instructions::transfer_operatorship_handler(ctx)
    }
}
