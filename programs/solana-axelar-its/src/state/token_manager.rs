use crate::{errors::ItsError, state::FlowState};
use anchor_lang::prelude::*;
use anchor_spl::token_2022::spl_token_2022::{
    extension::{BaseStateWithExtensions, ExtensionType::TransferFeeConfig, StateWithExtensions},
    state::Mint as SplMint,
};

#[account]
#[derive(Debug, Eq, PartialEq, InitSpace)]
pub struct TokenManager {
    /// The type of `TokenManager`.
    pub ty: Type,

    /// The interchain token id.
    pub token_id: [u8; 32],

    /// The token address within the Solana chain.
    pub token_address: Pubkey,

    /// The associated token account owned by the token manager.
    pub associated_token_account: Pubkey,

    /// The flow limit for the token manager.
    pub flow_slot: FlowState,

    /// The token manager PDA bump seed.
    pub bump: u8,
}

impl TokenManager {
    pub const SEED_PREFIX: &'static [u8] = b"token-manager";

    pub fn pda_seeds<'a>(token_id: &'a [u8; 32], its_root_pda: &'a Pubkey) -> [&'a [u8]; 3] {
        [Self::SEED_PREFIX, its_root_pda.as_ref(), token_id]
    }

    pub fn try_find_pda(token_id: [u8; 32], its_root_pda: Pubkey) -> Option<(Pubkey, u8)> {
        Pubkey::try_find_program_address(&Self::pda_seeds(&token_id, &its_root_pda), &crate::ID)
    }

    pub fn find_pda(token_id: [u8; 32], its_root_pda: Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&Self::pda_seeds(&token_id, &its_root_pda), &crate::ID)
    }

    pub fn find_token_mint(token_id: [u8; 32], its_root_pda: Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[
                crate::seed_prefixes::INTERCHAIN_TOKEN_SEED,
                its_root_pda.as_ref(),
                &token_id,
            ],
            &crate::ID,
        )
    }

    pub fn find_token_metadata(token_id: [u8; 32], its_root_pda: Pubkey) -> (Pubkey, u8) {
        let token_mint = Self::find_token_mint(token_id, its_root_pda).0;

        mpl_token_metadata::accounts::Metadata::find_pda(&token_mint)
    }

    /// Initializes a `TokenManager` account with given values.
    pub fn init_account(
        account: &mut Account<Self>,
        token_manager_type: Type,
        token_id: [u8; 32],
        token_address: Pubkey,
        associated_token_account: Pubkey,
        bump: u8,
    ) {
        account.ty = token_manager_type;
        account.token_id = token_id;
        account.token_address = token_address;
        account.associated_token_account = associated_token_account;
        account.flow_slot = FlowState::new(None, 0);
        account.bump = bump;
    }
}

// TODO rename this to TokenManagerType or similar
// to avoid ambiguity
#[derive(Debug, Eq, PartialEq, Clone, Copy, AnchorSerialize, AnchorDeserialize, InitSpace)]
pub enum Type {
    /// For tokens that are deployed directly from ITS itself they use a native
    /// interchain token manager. Tokens that are deployed via the frontend
    /// portal also use this type of manager.
    NativeInterchainToken,

    /// The mint/burnFrom token manager type, allows tokens to be burnt on the
    /// source chain when they are transferred out of that chain and minted they
    /// are transferred back into the source chain. As the name suggests when
    /// the token is burnt on the source chain the manager is looking to trigger
    /// the `burnFrom` function on the token rather than the `burn` function.
    /// The main implication is that ITS must be approved to call `burnFrom` by
    /// the token. The manager must be granted the role to be able to `mint` the
    /// token on the destination chain.
    MintBurnFrom,

    /// Token integrations using the lock/unlock token manager will have their
    /// token locked with their token’s manager. Only a single lock/unlock
    /// manager can exist for a token as having multiple lock/unlock managers
    /// would make it substantially more difficult to manage liquidity across
    /// many different blockchains. These token managers are best used in the
    /// case where a token has a “home chain” where a token can be locked. On
    /// the remote chains users can then use a wrapped version of that token
    /// which derives it’s value from a locked token back on the home chain.
    /// Canonical tokens for example deployed via ITS are examples where a
    /// lock/unlock token manager type is useful. When bridging tokens out of
    /// the destination chain (locking them at the manager) ITS will call the
    /// `transferTokenFrom` function, which in turn will call the
    /// `safeTransferFrom` function. For this transaction to be successful, ITS
    /// must be `approved` to call the `safeTransferFrom` function, otherwise
    /// the call will revert.
    LockUnlock,

    /// This manager type is similar to the lock/unlock token manager, where the
    /// manager locks
    /// the token on it’s “home chain” when it is bridged out and unlocks it
    /// when it is bridged back. The key feature with this token manager is
    /// that you have the option to set a fee that will be deducted when
    /// executing an `interchainTransfer`.
    ///
    /// This token type is currently not supported.
    LockUnlockFee,

    /// The mint/burn token manager type is the most common token manager type
    /// used for integrating tokens to ITS. This token manager type is used when
    /// there is no home chain for your token and allows you to `burn` tokens
    /// from the source chain and `mint` tokens on the destination chain. The
    /// manager will need to be granted the role to be able to execute the
    /// `mint` and `burn` function on the token.
    MintBurn,
}

impl Type {
    pub fn supports_mint_extensions(
        &self,
        token_mint: StateWithExtensions<'_, SplMint>,
    ) -> Result<bool> {
        let has_transfer_fee = token_mint
            .get_extension_types()?
            .contains(&TransferFeeConfig);

        match (self, has_transfer_fee) {
            (Self::LockUnlock, true) | (Self::LockUnlockFee, false) => Ok(false),
            _ => Ok(true),
        }
    }

    pub fn assert_supports_mint_extensions(
        &self,
        token_mint: StateWithExtensions<'_, SplMint>,
    ) -> Result<()> {
        if !self.supports_mint_extensions(token_mint)? {
            return Err(error!(ItsError::TokenManagerMintExtensionMismatch));
        }
        Ok(())
    }
}

impl TryFrom<u8> for Type {
    type Error = ProgramError;

    fn try_from(value: u8) -> std::result::Result<Self, Self::Error> {
        let converted = match value {
            0 => Self::NativeInterchainToken,
            1 => Self::MintBurnFrom,
            2 => Self::LockUnlock,
            3 => Self::LockUnlockFee,
            4 => Self::MintBurn,
            _ => return Err(ProgramError::InvalidInstructionData),
        };

        Ok(converted)
    }
}

// 32-byte little-endian representation
impl From<Type> for [u8; 32] {
    fn from(value: Type) -> Self {
        let mut bytes = [0u8; 32];
        bytes[0] = match value {
            Type::NativeInterchainToken => 0,
            Type::MintBurnFrom => 1,
            Type::LockUnlock => 2,
            Type::LockUnlockFee => 3,
            Type::MintBurn => 4,
        };
        bytes
    }
}

impl From<Type> for u8 {
    fn from(value: Type) -> Self {
        match value {
            Type::NativeInterchainToken => 0,
            Type::MintBurnFrom => 1,
            Type::LockUnlock => 2,
            Type::LockUnlockFee => 3,
            Type::MintBurn => 4,
        }
    }
}
