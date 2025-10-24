use crate::{
    errors::ITSError,
    events::{InterchainTokenDeployed, InterchainTokenIdClaimed},
    seed_prefixes::{INTERCHAIN_TOKEN_SEED, TOKEN_MANAGER_SEED},
    state::{InterchainTokenService, TokenManager},
    utils::{interchain_token_deployer_salt, interchain_token_id, interchain_token_id_internal},
};
use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use mpl_token_metadata::{instructions::CreateV1CpiBuilder, types::TokenStandard};

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct DeployInterchainTokenData {
    pub salt: [u8; 32],
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
    pub initial_supply: u64,
    pub minter: Option<Pubkey>,
}

#[derive(Accounts)]
#[event_cpi]
#[instruction(params: DeployInterchainTokenData)]
pub struct DeployInterchainToken<'info> {
    /// Payer for the transaction and account initialization
    #[account(mut)]
    pub payer: Signer<'info>,

    /// The deployer of the token (must sign the transaction)
    pub deployer: Signer<'info>,

    /// System program
    pub system_program: Program<'info, System>,

    /// ITS root configuration PDA
    #[account(
        seeds = [InterchainTokenService::SEED_PREFIX],
        bump = its_root_pda.bump,
        constraint = !its_root_pda.paused @ ITSError::Paused
    )]
    pub its_root_pda: Account<'info, InterchainTokenService>,

    /// Token Manager PDA for this token
    #[account(
        init,
        payer = payer,
        space = 8 + std::mem::size_of::<TokenManager>(),
        seeds = [
            TOKEN_MANAGER_SEED,
            its_root_pda.key().as_ref(),
            &interchain_token_id(&deployer.key(), &params.salt)
        ],
        bump
    )]
    pub token_manager_pda: Account<'info, TokenManager>,

    /// The mint account for the new token (Token-2022 compatible)
    #[account(
        init,
        payer = payer,
        mint::decimals = params.decimals,
        mint::authority = token_manager_pda,
        mint::freeze_authority = token_manager_pda,
        mint::token_program = token_program,
        seeds = [
            INTERCHAIN_TOKEN_SEED,
            its_root_pda.key().as_ref(),
            &interchain_token_id(&deployer.key(), &params.salt)
        ],
        bump,
    )]
    pub token_mint: InterfaceAccount<'info, Mint>,

    /// Token Manager’s associated token account (ATA)
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

    /// Sysvar for instructions (used by Metaplex)
    #[account(address = anchor_lang::solana_program::sysvar::instructions::id())]
    pub sysvar_instructions: UncheckedAccount<'info>,

    /// Metaplex Token Metadata program
    #[account(address = mpl_token_metadata::programs::MPL_TOKEN_METADATA_ID)]
    pub mpl_token_metadata_program: UncheckedAccount<'info>,

    /// Metadata account for the token
    #[account(
        mut,
        seeds = [
            b"metadata",
            mpl_token_metadata::programs::MPL_TOKEN_METADATA_ID.as_ref(),
            token_mint.key().as_ref()
        ],
        bump,
        seeds::program = mpl_token_metadata::programs::MPL_TOKEN_METADATA_ID
    )]
    pub mpl_token_metadata_account: UncheckedAccount<'info>,

    /// Deployer’s associated token account
    #[account(
        init,
        payer = payer,
        associated_token::mint = token_mint,
        associated_token::authority = deployer,
        associated_token::token_program = token_program
    )]
    pub deployer_ata: InterfaceAccount<'info, TokenAccount>,
}

pub fn deploy_interchain_token_handler(
    ctx: Context<DeployInterchainToken>,
    params: DeployInterchainTokenData,
) -> Result<()> {
    let deploy_salt = interchain_token_deployer_salt(ctx.accounts.deployer.key, &params.salt);
    let token_id = interchain_token_id_internal(&deploy_salt);
    let cpi_token_id = interchain_token_id(ctx.accounts.deployer.key, &params.salt);

    if params.initial_supply == 0 && params.minter.is_none() {
        return err!(ITSError::InvalidArgument);
    }

    if params.name.len() > mpl_token_metadata::MAX_NAME_LENGTH
        || params.symbol.len() > mpl_token_metadata::MAX_SYMBOL_LENGTH
    {
        return err!(ITSError::InvalidArgument);
    }

    emit_cpi!(InterchainTokenIdClaimed {
        token_id,
        deployer: *ctx.accounts.deployer.key,
        salt: deploy_salt,
    });

    initialize_token_manager(
        &mut ctx.accounts.token_manager_pda,
        token_id,
        *ctx.accounts.token_mint.to_account_info().key,
        *ctx.accounts.token_manager_ata.to_account_info().key,
        ctx.bumps.token_manager_pda,
    )?;

    create_token_metadata(
        &ctx.accounts,
        &params,
        cpi_token_id,
        ctx.bumps.token_manager_pda,
    )?;

    if params.initial_supply > 0 {
        mint_initial_supply(
            &ctx.accounts,
            cpi_token_id,
            params.initial_supply,
            ctx.bumps.token_manager_pda,
        )?;
    }

    emit_cpi!(InterchainTokenDeployed {
        token_id,
        token_address: *ctx.accounts.token_mint.to_account_info().key,
        minter: params.minter.unwrap_or_default(),
        name: params.name.clone(),
        symbol: params.symbol.clone(),
        decimals: params.decimals,
    });

    anchor_lang::solana_program::program::set_return_data(&token_id);

    Ok(())
}

fn initialize_token_manager(
    token_manager_pda: &mut Account<TokenManager>,
    token_id: [u8; 32],
    token_address: Pubkey,
    associated_token_account: Pubkey,
    bump: u8,
) -> Result<()> {
    use crate::state::{token_manager::Type, FlowState};

    token_manager_pda.ty = Type::NativeInterchainToken;
    token_manager_pda.token_id = token_id;
    token_manager_pda.token_address = token_address;
    token_manager_pda.associated_token_account = associated_token_account;
    token_manager_pda.flow_slot = FlowState::new(None, 0);
    token_manager_pda.bump = bump;

    Ok(())
}

fn mint_initial_supply<'info>(
    accounts: &DeployInterchainToken<'info>,
    token_id: [u8; 32],
    initial_supply: u64,
    token_manager_bump: u8,
) -> Result<()> {
    use anchor_spl::token_interface;

    let cpi_accounts = token_interface::MintTo {
        mint: accounts.token_mint.to_account_info(),
        to: accounts.deployer_ata.to_account_info(),
        authority: accounts.token_manager_pda.to_account_info(),
    };

    // Create signer seeds with proper lifetimes
    let its_root_key = accounts.its_root_pda.key();
    let bump_seed = [token_manager_bump];
    let signer_seeds: &[&[&[u8]]] = &[&[
        TOKEN_MANAGER_SEED,
        its_root_key.as_ref(),
        token_id.as_ref(),
        &bump_seed,
    ]];

    let cpi_context = CpiContext::new_with_signer(
        accounts.token_program.to_account_info(),
        cpi_accounts,
        signer_seeds,
    );

    token_interface::mint_to(cpi_context, initial_supply)?;

    Ok(())
}

fn create_token_metadata<'info>(
    accounts: &DeployInterchainToken<'info>,
    params: &DeployInterchainTokenData,
    token_id: [u8; 32],
    token_manager_bump: u8,
) -> Result<()> {
    // Truncate name and symbol to fit Metaplex limits
    let truncated_name = if params.name.len() > mpl_token_metadata::MAX_NAME_LENGTH {
        params.name[..mpl_token_metadata::MAX_NAME_LENGTH].to_string()
    } else {
        params.name.clone()
    };

    let truncated_symbol = if params.symbol.len() > mpl_token_metadata::MAX_SYMBOL_LENGTH {
        params.symbol[..mpl_token_metadata::MAX_SYMBOL_LENGTH].to_string()
    } else {
        params.symbol.clone()
    };

    // Create the token metadata using Metaplex CPI
    CreateV1CpiBuilder::new(&accounts.mpl_token_metadata_program.to_account_info())
        .metadata(&accounts.mpl_token_metadata_account.to_account_info())
        .token_standard(TokenStandard::Fungible)
        .mint(&accounts.token_mint.to_account_info(), false)
        .authority(&accounts.token_manager_pda.to_account_info())
        .update_authority(&accounts.token_manager_pda.to_account_info(), true)
        .payer(&accounts.payer.to_account_info())
        .is_mutable(false) // Make metadata immutable for interchain tokens
        .name(truncated_name)
        .symbol(truncated_symbol)
        .uri(String::new()) // Empty URI for now, can be customized later
        .seller_fee_basis_points(0) // No royalties for fungible tokens
        .system_program(&accounts.system_program.to_account_info())
        .sysvar_instructions(&accounts.sysvar_instructions.to_account_info())
        .invoke_signed(&[&[
            TOKEN_MANAGER_SEED,
            accounts.its_root_pda.key().as_ref(),
            token_id.as_ref(),
            &[token_manager_bump],
        ]])?;

    Ok(())
}
