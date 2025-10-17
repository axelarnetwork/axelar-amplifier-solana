use crate::{
    errors::ITSError,
    events::{InterchainTokenIdClaimed, TokenManagerDeployed},
    instructions::{get_token_metadata, validate_mint_extensions},
    seed_prefixes::TOKEN_MANAGER_SEED,
    state::{FlowState, InterchainTokenService, TokenManager, Type},
    utils::{
        canonical_interchain_token_deploy_salt, canonical_interchain_token_id,
        interchain_token_id_internal,
    },
};
use anchor_lang::prelude::*;
use anchor_spl::token_2022::spl_token_2022::extension::{
    BaseStateWithExtensions, ExtensionType, StateWithExtensions,
};
use anchor_spl::token_2022::spl_token_2022::state::Mint as SplMint;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::TokenAccount,
    token_interface::{Mint, TokenInterface},
};

#[derive(Accounts)]
#[event_cpi]
pub struct RegisterCanonicalInterchainToken<'info> {
    /// Payer for the transaction and account initialization
    #[account(mut)]
    pub payer: Signer<'info>,

    /// Metadata account for the token (required for canonical tokens)
    #[account(
        seeds = [
            b"metadata",
            mpl_token_metadata::programs::MPL_TOKEN_METADATA_ID.as_ref(),
            token_mint.key().as_ref()
        ],
        bump,
        seeds::program = mpl_token_metadata::programs::MPL_TOKEN_METADATA_ID
    )]
    pub metadata_account: UncheckedAccount<'info>,

    /// System program
    pub system_program: Program<'info, System>,

    /// ITS root configuration PDA
    #[account(
        seeds = [InterchainTokenService::SEED_PREFIX],
        bump = its_root_pda.bump,
        constraint = !its_root_pda.paused @ ITSError::Paused
    )]
    pub its_root_pda: Account<'info, InterchainTokenService>,

    /// Token Manager PDA for this canonical token
    #[account(
        init,
        payer = payer,
        space = TokenManager::DISCRIMINATOR.len() + std::mem::size_of::<TokenManager>(),
        seeds = [
            TOKEN_MANAGER_SEED,
            its_root_pda.key().as_ref(),
            &canonical_interchain_token_id(&token_mint.key())
        ],
        bump
    )]
    pub token_manager_pda: Account<'info, TokenManager>,

    /// The token mint to register as canonical
    pub token_mint: InterfaceAccount<'info, Mint>,

    /// Token Manager's associated token account (ATA)
    #[account(
        init,
        payer = payer,
        associated_token::mint = token_mint,
        associated_token::authority = token_manager_pda,
        associated_token::token_program = token_program
    )]
    pub token_manager_ata: InterfaceAccount<'info, TokenAccount>,

    /// Token program (can be SPL Token or Token-2022)
    pub token_program: Interface<'info, TokenInterface>,

    /// Associated token program
    pub associated_token_program: Program<'info, AssociatedToken>,

    /// Rent sysvar
    pub rent: Sysvar<'info, Rent>,
}

pub fn register_canonical_interchain_token_handler(
    ctx: Context<RegisterCanonicalInterchainToken>,
) -> Result<()> {
    msg!("Instruction: RegisterCanonicalInterchainToken");

    // Metadata is required for canonical tokens
    if let Err(_err) = get_token_metadata(
        &ctx.accounts.token_mint.to_account_info(),
        Some(&ctx.accounts.metadata_account),
    ) {
        return err!(ITSError::InvalidAccountData);
    }

    // Check if token has fee extension to determine manager type
    let token_mint_account = ctx.accounts.token_mint.to_account_info();
    let mint_data = token_mint_account.try_borrow_data()?;
    let mint = StateWithExtensions::<SplMint>::unpack(&mint_data)?;
    let has_fee_extension = mint
        .get_extension_types()?
        .contains(&ExtensionType::TransferFeeConfig);

    let token_manager_type = if has_fee_extension {
        Type::LockUnlockFee
    } else {
        Type::LockUnlock
    };

    validate_mint_extensions(
        token_manager_type,
        &ctx.accounts.token_mint.to_account_info(),
    )?;

    let deploy_salt = canonical_interchain_token_deploy_salt(&ctx.accounts.token_mint.key());
    let token_id = interchain_token_id_internal(&deploy_salt);

    emit_cpi!(InterchainTokenIdClaimed {
        token_id,
        deployer: *ctx.accounts.payer.key,
        salt: deploy_salt,
    });

    // Initialize the Token Manager
    let token_manager = &mut ctx.accounts.token_manager_pda;
    token_manager.ty = token_manager_type;
    token_manager.token_id = token_id;
    token_manager.token_address = *ctx.accounts.token_mint.to_account_info().key;
    token_manager.associated_token_account = *ctx.accounts.token_manager_ata.to_account_info().key;
    token_manager.flow_slot = FlowState::new(None, 0);
    token_manager.bump = ctx.bumps.token_manager_pda;

    emit_cpi!(TokenManagerDeployed {
        token_id,
        token_manager: *ctx.accounts.token_manager_pda.to_account_info().key,
        token_manager_type: token_manager_type.into(),
        params: Vec::new(), // No additional params for canonical tokens
    });

    // Set return data with token_id
    anchor_lang::solana_program::program::set_return_data(&token_id);

    Ok(())
}
