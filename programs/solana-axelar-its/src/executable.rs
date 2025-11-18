use anchor_lang::{prelude::*, InstructionData};
pub use solana_axelar_gateway::executable::*;

pub use mpl_token_metadata::accounts::Metadata as TokenMetadata;

#[allow(unused_imports)]
use crate as solana_axelar_its;

/// Anchor discriminator for the `execute_with_interchain_token` instruction.
/// sha256("global:execute_with_interchain_token")[..8]
///
/// Useful for when the execute instruction has a different name in the program.
/// # Example
/// ```
pub const ITS_EXECUTE_IX_DISC: &[u8; 8] = &[251, 218, 49, 130, 208, 58, 231, 44];

/// The index of the first account that is expected to be passed to the
/// destination program.
pub const ITS_EXECUTE_PROGRAM_ACCOUNTS_START_INDEX: usize = 5;

#[derive(Clone, Debug, AnchorSerialize, AnchorDeserialize)]
pub struct ExecuteWithInterchainTokenPayload {
    /// The unique message id.
    pub command_id: [u8; 32],
    /// The source chain of the token transfer.
    pub source_chain: String,
    /// The source address of the token transfer.
    pub source_address: Vec<u8>,
    /// The destination program
    pub destination_address: Pubkey,
    /// The token ID.
    pub token_id: [u8; 32],
    /// The token mint address.
    pub token_mint: Pubkey,
    /// Amount of tokens being transferred.
    pub amount: u64,
    /// The execution payload
    pub data: Vec<u8>,
}

impl Discriminator for ExecuteWithInterchainTokenPayload {
    const DISCRIMINATOR: &'static [u8] = ITS_EXECUTE_IX_DISC;
}
impl InstructionData for ExecuteWithInterchainTokenPayload {}

/// Holds references to the Axelar executable accounts needed for validation.
/// This is returned by the `HasAxelarExecutable` trait.
pub struct AxelarExecutableWithInterchainTokenAccountRefs<'a, 'info> {
    pub token_program: &'a AccountInfo<'info>,
    pub token_mint: &'a AccountInfo<'info>,
    pub destination_program_ata: &'a AccountInfo<'info>,
    pub interchain_transfer_execute: &'a AccountInfo<'info>,
}

/// Trait that must be implemented by account structs that contain Axelar executable accounts.
/// This trait is automatically implemented when using the `executable_with_interchain_token_accounts!` macro.
pub trait HasAxelarExecutableWithInterchainToken<'info> {
    fn axelar_executable_with_interchain_token(
        &self,
    ) -> AxelarExecutableWithInterchainTokenAccountRefs<'_, 'info>;
}

//
// Accounts
//

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
            self.interchain_transfer_execute.clone(),
            self.token_program.clone(),
            self.token_mint.clone(),
            self.destination_program_ata.clone(),
        ]
    }
}

/// NOTE: Keep in mind the outer accounts struct must not include:
/// ```ignore
/// #[instruction(message: Message, payload: Vec<u8>)]
/// ```
/// attribute due to [a bug](https://github.com/solana-foundation/anchor/issues/2942) in Anchor.
// NOTE: This macro is necessary because Anchor currently does not support importing
// accounts from other crates. Once Anchor supports this, we can remove this macro and
// export the accounts directly from solana-axelar-gateway.
// See: https://github.com/solana-foundation/anchor/issues/3811
// It is also not possible to use the `cpi` module inside the gateway crate.
#[macro_export]
#[allow(clippy::crate_in_macro_def)]
macro_rules! executable_with_interchain_token_accounts {
    ($outer_struct:ident) => {
    /// Accounts for executing an inbound Axelar GMP message.
    /// NOTE: Keep in mind the outer accounts struct must not include:
    /// ```ignore
    /// #[instruction(message: Message, payload: Vec<u8>)]
    /// ```
    /// attribute due to [a bug](https://github.com/solana-foundation/anchor/issues/2942) in Anchor.
    #[derive(Accounts)]
    #[instruction(execute_payload: solana_axelar_its::executable::ExecuteWithInterchainTokenPayload,)]
    pub struct AxelarExecuteWithInterchainTokenAccounts<'info> {
	  	pub token_program: Interface<'info, anchor_spl::token_interface::TokenInterface>,

	    #[account(mint::token_program = token_program)]
	    pub token_mint: InterfaceAccount<'info, anchor_spl::token_interface::Mint>,

	    #[account(
	        associated_token::mint = token_mint,
            associated_token::authority = crate::ID,
            associated_token::token_program = token_program,
		)]
	    pub destination_program_ata: AccountInfo<'info>,

		#[account(
			seeds = [solana_axelar_its::state::InterchainTransferExecute::SEED_PREFIX, crate::ID.as_ref()],
            bump,
            seeds::program = solana_axelar_its::ID,
		)]
	    pub interchain_transfer_execute: Signer<'info>,
    }

    impl<'info> solana_axelar_its::executable::HasAxelarExecutableWithInterchainToken<'info> for $outer_struct<'info> {
        fn axelar_executable_with_interchain_token(&self) -> solana_axelar_its::executable::AxelarExecutableWithInterchainTokenAccountRefs<'_, 'info> {
            (&self.its_executable).into()
        }
    }

    impl<'a, 'info> From<&'a AxelarExecuteWithInterchainTokenAccounts<'info>> for solana_axelar_its::executable::AxelarExecutableWithInterchainTokenAccountRefs<'a, 'info> {
        fn from(accounts: &'a AxelarExecuteWithInterchainTokenAccounts<'info>) -> Self {
            Self {
                token_program: &accounts.token_program,
                token_mint: &accounts.token_mint.to_account_info(),
                destination_program_ata: &accounts.destination_program_ata,
                interchain_transfer_execute: &accounts.interchain_transfer_execute,
            }
        }
    }

    };
}
