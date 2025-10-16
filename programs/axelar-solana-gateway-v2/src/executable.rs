use anchor_lang::prelude::*;

// Re-export Message
pub use crate::Message;
pub use axelar_message_primitives::DataPayload as ExecutablePayload;
pub use axelar_message_primitives::EncodingScheme as ExecutablePayloadEncodingScheme;

/// Anchor discriminator for the `execute` instruction.
/// sha256("global:execute")[..8]
///
/// Useful for when the execute instruction has a different name in the program.
/// # Example
/// ```ignore
/// use anchor_lang::prelude::*;
/// use axelar_solana_gateway_v2::executable::EXECUTE_IX_DISC;
/// use axelar_solana_gateway_v2::Message;
///
/// declare_id!("YourProgramId11111111111111111111111111111");
///
/// #[program]
/// pub mod your_program {
///     use super::*;
///
///     #[instruction(discriminator = EXECUTE_IX_DISC)]
///     pub fn process_gmp(
///         ctx: Context<ProcessGmp>,
///         message: Message,
///         payload: Vec<u8>
///     ) -> Result<()> {
///         // Your GMP message handling logic
///         Ok(())
///     }
/// }
/// ```
pub const EXECUTE_IX_DISC: &[u8; 8] = &[130, 221, 242, 154, 13, 193, 189, 29];

/// Macro to generate executable accounts and validation function.
/// Usage:
/// ```ignore
/// use anchor_lang::prelude::*;
/// use axelar_solana_gateway_v2::{executable::*, executable_accounts};
///
/// executable_accounts!();
///
/// #[derive(Accounts)]
/// pub struct Execute<'info> {
///     // GMP Accounts
///     pub executable: AxelarExecuteAccounts<'info>,
///
///     // Your program accounts here
/// }
///
/// pub fn execute_handler(ctx: Context<Execute>, message: Message, payload: Vec<u8>) -> Result<()> {
///     validate_message(&ctx.accounts.executable, message, &payload)?;
///
///     Ok(())
/// }
/// ```
// NOTE: This macro is necessary because Anchor currently does not support importing
// accounts from other crates. Once Anchor supports this, we can remove this macro and
// export the accounts directly from axelar-solana-gateway-v2.
// See: https://github.com/solana-foundation/anchor/issues/3811
// It is also not possible to use the `cpi` module inside the gateway crate.
#[macro_export]
macro_rules! executable_accounts {
    () => {
    /// Accounts for executing an inbound Axelar GMP message.
    #[derive(Accounts)]
    #[instruction(message: Message)]
    pub struct AxelarExecuteAccounts<'info> {
        #[account(
            seeds = [axelar_solana_gateway_v2::IncomingMessage::SEED_PREFIX, message.command_id().as_ref()],
            bump = incoming_message_pda.load()?.bump,
            seeds::program = axelar_gateway_program.key()
        )]
        pub incoming_message_pda: AccountLoader<'info, axelar_solana_gateway_v2::IncomingMessage>,

        #[account(
            seeds = [axelar_solana_gateway_v2::seed_prefixes::VALIDATE_MESSAGE_SIGNING_SEED, message.command_id().as_ref()],
            bump = incoming_message_pda.load()?.signing_pda_bump,
        )]
        pub signing_pda: AccountInfo<'info>,

        pub axelar_gateway_program: Program<'info, axelar_solana_gateway_v2::program::AxelarSolanaGatewayV2>,

        #[account(
            seeds = [b"__event_authority"],
            bump,
            seeds::program = axelar_gateway_program.key()
        )]
        pub event_authority: SystemAccount<'info>,

        pub system_program: Program<'info, System>,
    }

    pub fn validate_message<'info>(
        executable_accounts: &AxelarExecuteAccounts<'info>,
        message: Message,
        payload: &[u8],
    ) -> Result<()> {
    	// Verify that the payload hash matches the computed hash of the payload
        let computed_payload_hash = anchor_lang::solana_program::keccak::hashv(&[payload]).to_bytes();
        if computed_payload_hash != message.payload_hash {
            return err!(ExecutableError::InvalidPayloadHash);
        }

        let cpi_accounts = axelar_solana_gateway_v2::cpi::accounts::ValidateMessage {
            incoming_message_pda: executable_accounts.incoming_message_pda.to_account_info(),
            caller: executable_accounts.signing_pda.to_account_info(),
            event_authority: executable_accounts.event_authority.to_account_info(),
            program: executable_accounts.axelar_gateway_program.to_account_info(),
        };

        // Prepare signer seeds
        let command_id = message.command_id();
        let signer_seeds = &[
            axelar_solana_gateway_v2::seed_prefixes::VALIDATE_MESSAGE_SIGNING_SEED,
            &command_id,
            &[executable_accounts
                .incoming_message_pda
                .load()?
                .signing_pda_bump],
        ];
        let signer_seeds = &[&signer_seeds[..]];

        // Create CPI context
        // with the signing PDA as the signer
        let cpi_ctx = CpiContext::new_with_signer(
            executable_accounts.axelar_gateway_program.to_account_info(),
            cpi_accounts,
            signer_seeds,
        );

        // Call the validate_message CPI
        axelar_solana_gateway_v2::cpi::validate_message(cpi_ctx, message)?;

        Ok(())
    }

    };
}

#[error_code]
pub enum ExecutableError {
    #[msg("Payload hash does not match the computed hash of the payload")]
    InvalidPayloadHash,
}
