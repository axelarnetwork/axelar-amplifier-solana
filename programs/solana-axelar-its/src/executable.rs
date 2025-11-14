use anchor_lang::{prelude::*, InstructionData};
#[allow(unused_imports)]
use solana_axelar_gateway::executable::*;

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
            AccountMeta::new(self.token_program.key(), false),
            AccountMeta::new(self.token_mint.key(), false),
            AccountMeta::new(self.destination_program_ata.key(), false),
            AccountMeta::new(
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
