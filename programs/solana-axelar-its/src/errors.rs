use anchor_lang::prelude::*;

#[error_code]
pub enum ItsError {
    #[msg("The ITS program is paused")]
    Paused,
    #[msg("Amount is invalid")]
    InvalidAmount,
    #[msg("Instruction argument is invalid")]
    InvalidArgument,
    #[msg("Invalid instruction data")]
    InvalidInstructionData,
    #[msg("Invalid Metaplex data account")]
    InvalidMetaplexDataAccount,
    #[msg("Minter account not provided")]
    MinterNotProvided,
    #[msg("Minter roles pda not provided")]
    MinterRolesNotProvided,
    #[msg("Minter roles pda bump not provided")]
    MinterRolesPdaBumpNotProvided,
    #[msg("Missing operator account")]
    OperatorNotProvided,
    #[msg("Missing operator roles pda")]
    OperatorRolesPdaNotProvided,
    #[msg("Account data is invalid")]
    InvalidAccountData,
    #[msg("The source chain name is untrusted")]
    UntrustedSourceChain,
    #[msg("The destination chain name is untrusted")]
    UntrustedDestinationChain,
    #[msg("The chain is already set as trusted")]
    TrustedChainAlreadySet,
    #[msg("The chain is not set as trusted")]
    TrustedChainNotSet,
    #[msg("The destination chain name is invalid")]
    InvalidDestinationChain,
    #[msg("The destination address is invalid")]
    InvalidDestinationAddress,
    #[msg("The destination address account is invalid")]
    InvalidDestinationAddressAccount,
    #[msg("The token mint cannot have fixed zero supply")]
    ZeroSupplyToken,
    #[msg("The mint extension is not compatible with the TokenManager type")]
    TokenManagerMintExtensionMismatch,
    #[msg("The signer is not a user account")]
    CallerNotUserAccount,
    #[msg("The token mint is invalid")]
    InvalidTokenMint,
    #[msg("Token manager missmatch with token mint")]
    TokenMintTokenManagerMissmatch,
    #[msg("Missing remaining account in execute instruction")]
    AccountNotProvided,
    #[msg("source_id and pda_seeds must both be provided together or both be None")]
    InconsistentSourceIdAndPdaSeeds,
    #[msg("Arithmetic overflow")]
    ArithmeticOverflow,
    #[msg("Missing required signature")]
    MissingRequiredSignature,
    #[msg("Not enough account keys")]
    NotEnoughAccountKeys,
    #[msg("Invalid token manager type")]
    InvalidTokenManagerType,
}
