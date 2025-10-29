use crate::{
    errors::ItsError,
    events::{InterchainTokenDeployed, TokenManagerDeployed},
    instructions::validate_mint_extensions,
    seed_prefixes::{INTERCHAIN_TOKEN_SEED, TOKEN_MANAGER_SEED},
    state::{InterchainTokenService, Roles, TokenManager, Type, UserRoles},
};
use anchor_lang::prelude::*;
use anchor_spl::token_interface::TokenAccount;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{Mint, TokenInterface},
};
use mpl_token_metadata::{instructions::CreateV1CpiBuilder, types::TokenStandard};

#[derive(Accounts)]
#[event_cpi]
#[instruction(token_id: [u8; 32], name: String, symbol: String, decimals: u8)]
pub struct DeployInterchainTokenInternal<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    pub deployer: Signer<'info>,

    pub system_program: Program<'info, System>,

    #[account(
        seeds = [InterchainTokenService::SEED_PREFIX],
        bump = its_root_pda.bump,
        constraint = !its_root_pda.paused @ ItsError::Paused,
        signer // important: only ITS can call this
    )]
    pub its_root_pda: Account<'info, InterchainTokenService>,

    #[account(
        init,
        payer = payer,
        space = TokenManager::DISCRIMINATOR.len() + TokenManager::INIT_SPACE,
        seeds = [
            TOKEN_MANAGER_SEED,
            its_root_pda.key().as_ref(),
            &token_id
        ],
        bump
    )]
    pub token_manager_pda: Account<'info, TokenManager>,

    #[account(
        init,
        payer = payer,
        mint::decimals = decimals,
        mint::authority = token_manager_pda,
        mint::freeze_authority = token_manager_pda,
        mint::token_program = token_program,
        seeds = [
            INTERCHAIN_TOKEN_SEED,
            its_root_pda.key().as_ref(),
            &token_id
        ],
        bump,
    )]
    pub token_mint: InterfaceAccount<'info, Mint>,

    #[account(
        init,
        payer = payer,
        associated_token::mint = token_mint,
        associated_token::authority = token_manager_pda,
        associated_token::token_program = token_program
    )]
    pub token_manager_ata: InterfaceAccount<'info, TokenAccount>,
    #[account(address = anchor_spl::token_2022::ID)]
    pub token_program: Interface<'info, TokenInterface>,
    #[account(address = anchor_spl::associated_token::ID)]
    pub associated_token_program: Program<'info, AssociatedToken>,

    pub rent: Sysvar<'info, Rent>,

    #[account(address = anchor_lang::solana_program::sysvar::instructions::id())]
    pub sysvar_instructions: UncheckedAccount<'info>,

    #[account(address = mpl_token_metadata::programs::MPL_TOKEN_METADATA_ID)]
    pub mpl_token_metadata_program: UncheckedAccount<'info>,

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

    #[account(
        init,
        payer = payer,
        associated_token::mint = token_mint,
        associated_token::authority = deployer,
        associated_token::token_program = token_program
    )]
    pub deployer_ata: InterfaceAccount<'info, TokenAccount>,

    // Optional accounts
    pub minter: Option<UncheckedAccount<'info>>,
    #[account(
        init,
        payer = payer,
        space = UserRoles::DISCRIMINATOR.len() + UserRoles::INIT_SPACE,
        seeds = [
            UserRoles::SEED_PREFIX,
            token_manager_pda.key().as_ref(),
            minter.as_ref().unwrap().key().as_ref()
        ],
        bump
    )]
    pub minter_roles_pda: Option<Account<'info, UserRoles>>,
}

pub fn deploy_interchain_token_internal_handler(
    ctx: Context<DeployInterchainTokenInternal>,
    token_id: [u8; 32],
    name: String,
    symbol: String,
    decimals: u8,
) -> Result<()> {
    msg!("deploy_interchain_token_internal_handler");

    if name.len() > mpl_token_metadata::MAX_NAME_LENGTH
        || symbol.len() > mpl_token_metadata::MAX_SYMBOL_LENGTH
    {
        msg!("Name and/or symbol length too long");
        return err!(ItsError::InvalidArgument);
    }

    // Call process_inbound_deploy directly with the context accounts
    process_inbound_deploy(
        ctx.accounts,
        token_id,
        &name,
        &symbol,
        0,
        ctx.bumps.token_manager_pda,
        ctx.bumps.minter_roles_pda,
    )?;

    emit_cpi!(TokenManagerDeployed {
        token_id,
        token_manager: ctx.accounts.token_manager_pda.key(),
        token_manager_type: Type::NativeInterchainToken.into(),
        params: ctx
            .accounts
            .minter
            .as_ref()
            .map(|account| account.key().to_bytes().to_vec())
            .unwrap_or_default(),
    });

    emit_cpi!(InterchainTokenDeployed {
        token_id,
        token_address: ctx.accounts.token_mint.key(),
        minter: ctx
            .accounts
            .minter
            .as_ref()
            .map(anchor_lang::Key::key)
            .unwrap_or_default(),
        name: name.clone(),
        symbol: symbol.clone(),
        decimals,
    });

    Ok(())
}

pub fn process_inbound_deploy(
    ctx: &mut DeployInterchainTokenInternal,
    token_id: [u8; 32],
    name: &str,
    symbol: &str,
    initial_supply: u64,
    token_manager_pda_bump: u8,
    minter_roles_pda_bump: Option<u8>,
) -> Result<()> {
    // setup_mint
    if initial_supply > 0 {
        mint_initial_supply(ctx, token_id, initial_supply, token_manager_pda_bump)?;
    }

    // setup_metadata
    create_token_metadata(ctx, name, symbol, token_id, token_manager_pda_bump)?;

    // super::token_manager::deploy(...)
    validate_mint_extensions(
        Type::NativeInterchainToken,
        &ctx.token_mint.to_account_info(),
    )?;

    TokenManager::init_account(
        &mut ctx.token_manager_pda,
        Type::NativeInterchainToken,
        token_id,
        ctx.token_mint.key(),
        ctx.token_manager_ata.key(),
        token_manager_pda_bump,
    );

    // Initialize UserRoles
    if ctx.minter.is_some() && ctx.minter_roles_pda.is_some() {
        let minter_roles_pda = ctx.minter_roles_pda.as_mut().unwrap();
        minter_roles_pda.bump = minter_roles_pda_bump.unwrap();
        minter_roles_pda.roles = Roles::OPERATOR | Roles::FLOW_LIMITER | Roles::MINTER;
    }

    Ok(())
}

fn mint_initial_supply<'info>(
    accounts: &DeployInterchainTokenInternal<'info>,
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
    accounts: &DeployInterchainTokenInternal<'info>,
    name: &str,
    symbol: &str,
    token_id: [u8; 32],
    token_manager_bump: u8,
) -> Result<()> {
    let mut truncated_name = name.to_owned();
    let mut truncated_symbol = symbol.to_owned();
    truncated_name.truncate(mpl_token_metadata::MAX_NAME_LENGTH);
    truncated_symbol.truncate(mpl_token_metadata::MAX_SYMBOL_LENGTH);

    // Create the token metadata using Metaplex CPI
    CreateV1CpiBuilder::new(&accounts.mpl_token_metadata_program.to_account_info())
        .metadata(&accounts.mpl_token_metadata_account.to_account_info())
        .token_standard(TokenStandard::Fungible)
        .mint(&accounts.token_mint.to_account_info(), false)
        .authority(&accounts.token_manager_pda.to_account_info())
        .update_authority(&accounts.token_manager_pda.to_account_info(), true)
        .payer(&accounts.payer.to_account_info())
        .is_mutable(false)
        .name(truncated_name)
        .symbol(truncated_symbol)
        .uri(String::new())
        .seller_fee_basis_points(0)
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
