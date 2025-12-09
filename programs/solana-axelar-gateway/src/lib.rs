#![allow(clippy::empty_structs_with_brackets)]
// Anchor's #[program] macro generates code using deprecated AccountInfo::realloc
#![allow(deprecated)]

use anchor_lang::prelude::*;

pub mod instructions;
pub use instructions::*;

pub mod state;
pub use state::*;

pub mod events;
pub use events::*;

pub mod errors;
pub use errors::*;

pub mod executable;

pub mod payload;

use program_utils::ensure_single_feature;

pub use solana_axelar_std::Message;

ensure_single_feature!("devnet-amplifier", "stagenet", "testnet", "mainnet");

#[cfg(feature = "devnet-amplifier")]
declare_id!("gtwiF7Bamsq5zBQCnKjK7kHfmBQe7StQE9VPucWrtmA");

#[cfg(feature = "stagenet")]
declare_id!("gtwpfz1SLfPr1zmackMVMgShjkuCGPZ5taN8wAfwreW");

#[cfg(feature = "testnet")]
declare_id!("gtwpFGXoWNNMMaYGhJoNRMNAp8R3srFeBmKAoeLgSYy");

#[cfg(feature = "mainnet")]
declare_id!("gtw1111111111111111111111111111111111111111");

/// Seed prefixes for different PDAs initialized by the Gateway
pub mod seed_prefixes {
    use super::state;

    /// The seed prefix for deriving Gateway Config PDA
    pub const GATEWAY_SEED: &[u8] = state::GatewayConfig::SEED_PREFIX;
    /// The seed prefix for deriving `VerifierSetTracker` PDAs
    pub const VERIFIER_SET_TRACKER_SEED: &[u8] = state::VerifierSetTracker::SEED_PREFIX;
    /// The seed prefix for deriving signature verification PDAs
    pub const SIGNATURE_VERIFICATION_SEED: &[u8] =
        state::SignatureVerificationSessionData::SEED_PREFIX;
    /// The seed prefix for deriving call contract signature verification PDAs
    pub const CALL_CONTRACT_SIGNING_SEED: &[u8] = b"gtw-call-contract";
    /// The seed prefix for deriving incoming message PDAs
    pub const INCOMING_MESSAGE_SEED: &[u8] = state::IncomingMessage::SEED_PREFIX;
    /// The seed prefix for deriving validate message signing PDAs
    pub const VALIDATE_MESSAGE_SIGNING_SEED: &[u8] =
        state::IncomingMessage::VALIDATE_MESSAGE_SEED_PREFIX;
}

#[program]
pub mod solana_axelar_gateway {
    use super::*;

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
        ctx: Context<InitializeConfig>,
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
        verifier_info: solana_axelar_std::SigningVerifierSetInfo,
    ) -> Result<()> {
        instructions::verify_signature_handler(ctx, payload_merkle_root, verifier_info)
    }

    pub fn approve_message(
        ctx: Context<ApproveMessage>,
        merklized_message: solana_axelar_std::MerklizedMessage,
        payload_merkle_root: [u8; 32],
    ) -> Result<()> {
        instructions::approve_message_handler(ctx, merklized_message, payload_merkle_root)
    }

    pub fn validate_message(
        ctx: Context<ValidateMessage>,
        message: solana_axelar_std::Message,
    ) -> Result<()> {
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
