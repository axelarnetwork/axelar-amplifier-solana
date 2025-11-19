use anchor_lang::{prelude::*, InstructionData};
pub use solana_axelar_gateway::executable::*;

//
// Instruction
//

/// Anchor discriminator for the `execute_with_interchain_token` instruction.
/// sha256("global:execute_with_interchain_token")[..8]
///
/// Useful for when the execute instruction has a different name in the program.
/// # Example
/// ```ignore
/// use anchor_lang::prelude::*;
/// use solana_axelar_its::executable::*;
///
/// declare_id!("YourProgramId11111111111111111111111111111");
///
/// #[program]
/// pub mod your_program {
///     use super::*;
///
///     #[instruction(discriminator = ITS_EXECUTE_IX_DISC)]
///     pub fn receive_interchain_token(
///         ctx: Context<ReceiveToken>,
///         execute_payload: AxelarExecuteWithInterchainTokenPayload,
///     ) -> Result<()> {
///         // Your handling logic
///         Ok(())
///     }
/// }
/// ```
pub const ITS_EXECUTE_IX_DISC: &[u8; 8] = &[251, 218, 49, 130, 208, 58, 231, 44];

#[derive(Clone, Debug, AnchorSerialize, AnchorDeserialize)]
pub struct AxelarExecuteWithInterchainTokenPayload {
    /// The unique message id.
    pub command_id: [u8; 32],
    /// The source chain of the token transfer.
    pub source_chain: String,
    /// The source address of the token transfer.
    pub source_address: Vec<u8>,
    /// The token ID.
    pub token_id: [u8; 32],
    /// The token mint address.
    pub token_mint: Pubkey,
    /// Amount of tokens being transferred.
    pub amount: u64,
    /// The execution payload
    pub data: Vec<u8>,
}

#[derive(Clone, Debug, AnchorSerialize, AnchorDeserialize)]
pub struct AxelarExecuteWithInterchainTokenInstruction {
    pub execute_payload: AxelarExecuteWithInterchainTokenPayload,
}

impl Discriminator for AxelarExecuteWithInterchainTokenInstruction {
    const DISCRIMINATOR: &'static [u8] = ITS_EXECUTE_IX_DISC;
}
impl InstructionData for AxelarExecuteWithInterchainTokenInstruction {}

//
// Accounts
//

/// Macro to generate accounts for execute with interchain token.
/// Usage:
/// ```ignore
/// use anchor_lang::prelude::*;
/// use solana_axelar_its::{executable::*, executable_with_interchain_token_accounts};
///
/// executable_with_interchain_token_accounts!(ExecuteWithInterchainToken);
///
/// #[derive(Accounts)]
/// pub struct ExecuteWithInterchainToken<'info> {
///     pub its_executable: AxelarExecuteWithInterchainTokenAccounts<'info>,
///
///     // Your program accounts here
/// }
///
/// pub fn execute_with_interchain_token_handler(ctx: Context<ExecuteWithInterchainToken>, execute_payload: AxelarExecuteWithInterchainTokenPayload) -> Result<()> {
///     // Your handling logic here
///     Ok(())
/// }
/// ```
// NOTE: the `crate::ID` is used to refer to the current program ID in the macro.
// By checking the `interchain_transfer_execute` we verify the integrity of the caller.
#[macro_export]
#[allow(clippy::crate_in_macro_def)]
macro_rules! executable_with_interchain_token_accounts {
    ($outer_struct:ident) => {
    /// Accounts for executing a destination program with an interchain token.
    #[derive(Accounts)]
    pub struct AxelarExecuteWithInterchainTokenAccounts<'info> {
      	pub token_program: Interface<'info, anchor_spl::token_interface::TokenInterface>,

        #[account(mint::token_program = token_program)]
        pub token_mint: InterfaceAccount<'info, anchor_spl::token_interface::Mint>,

        #[account(
            associated_token::mint = token_mint,
            associated_token::authority = crate::ID,
            associated_token::token_program = token_program,
        )]
        pub destination_program_ata: InterfaceAccount<'info, anchor_spl::token_interface::TokenAccount>,

        #[account(
            seeds = [solana_axelar_its::state::InterchainTransferExecute::SEED_PREFIX, crate::ID.as_ref()],
            bump,
            seeds::program = solana_axelar_its::ID,
        )]
        pub interchain_transfer_execute: Signer<'info>,
    }

    };
}

/// Builder for AxelarExecuteWithInterchainToken instruction accounts.
pub mod builder {
    use super::*;

    pub struct AxelarExecuteWithInterchainToken<'info> {
        pub token_program: AccountInfo<'info>,
        pub token_mint: AccountInfo<'info>,
        pub destination_program_ata: AccountInfo<'info>,
        pub interchain_transfer_execute: AccountInfo<'info>,
    }

    impl<'info> anchor_lang::ToAccountMetas for AxelarExecuteWithInterchainToken<'info> {
        fn to_account_metas(&self, is_signer: Option<bool>) -> Vec<AccountMeta> {
            vec![
                AccountMeta::new_readonly(self.token_program.key(), false),
                AccountMeta::new(self.token_mint.key(), false),
                AccountMeta::new(self.destination_program_ata.key(), false),
                AccountMeta::new_readonly(
                    self.interchain_transfer_execute.key(),
                    is_signer.unwrap_or(false),
                ),
            ]
        }
    }

    impl<'info> anchor_lang::ToAccountInfos<'info> for AxelarExecuteWithInterchainToken<'info> {
        fn to_account_infos(&self) -> Vec<AccountInfo<'info>> {
            vec![
                self.token_program.clone(),
                self.token_mint.clone(),
                self.destination_program_ata.clone(),
                self.interchain_transfer_execute.clone(),
            ]
        }
    }
}
