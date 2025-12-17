use crate::{
    errors::ItsError,
    events::{InterchainTokenDeployed, TokenManagerDeployed},
    seed_prefixes::{INTERCHAIN_TOKEN_SEED, TOKEN_MANAGER_SEED},
    state::{roles, InterchainTokenService, TokenManager, Type, UserRoles},
    utils::truncate_utf8,
};
use anchor_lang::prelude::*;
use anchor_spl::{associated_token::AssociatedToken, token_interface::Mint};
use anchor_spl::{token_2022::Token2022, token_interface::TokenAccount};
use mpl_token_metadata::{instructions::CreateV1CpiBuilder, types::TokenStandard};

#[derive(Accounts)]
#[event_cpi]
#[instruction(token_id: [u8; 32], name: String, symbol: String, decimals: u8)]
pub struct ExecuteDeployInterchainToken<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

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
        init_if_needed,
        payer = payer,
        associated_token::mint = token_mint,
        associated_token::authority = token_manager_pda,
        associated_token::token_program = token_program
    )]
    pub token_manager_ata: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Program<'info, Token2022>,

    pub associated_token_program: Program<'info, AssociatedToken>,

    /// CHECK:
    #[account(address = solana_sdk_ids::sysvar::instructions::id())]
    pub sysvar_instructions: UncheckedAccount<'info>,

    /// CHECK:
    #[account(address = mpl_token_metadata::programs::MPL_TOKEN_METADATA_ID)]
    pub mpl_token_metadata_program: UncheckedAccount<'info>,

    /// CHECK:
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

    // Optional accounts
    /// CHECK:
    pub minter: Option<UncheckedAccount<'info>>,

    #[account(
        init,
        payer = payer,
        space = UserRoles::DISCRIMINATOR.len() + UserRoles::INIT_SPACE,
        seeds = [
            UserRoles::SEED_PREFIX,
            token_manager_pda.key().as_ref(),
            minter.as_ref().ok_or(ItsError::MinterNotProvided)?.key().as_ref()
        ],
        bump
    )]
    pub minter_roles_pda: Option<Account<'info, UserRoles>>,
}

pub fn execute_deploy_interchain_token_handler(
    ctx: Context<ExecuteDeployInterchainToken>,
    token_id: [u8; 32],
    name: String,
    symbol: String,
    decimals: u8,
    minter: Vec<u8>,
) -> Result<()> {
    // Truncate name and symbol for incoming deployments
    // to prevent metadata CPI failure
    let mut truncated_name = name;
    let mut truncated_symbol = symbol;
    truncate_utf8(&mut truncated_name, mpl_token_metadata::MAX_NAME_LENGTH);
    truncate_utf8(&mut truncated_symbol, mpl_token_metadata::MAX_SYMBOL_LENGTH);

    match (
        minter.is_empty(),
        &ctx.accounts.minter,
        &ctx.accounts.minter_roles_pda,
    ) {
        (true, None, None) => {
            // Valid: No minter specified
        }
        (false, Some(minter_account), Some(_)) => {
            // Valid: Minter specified with both accounts
            if minter_account.key().to_bytes().as_ref() != minter.as_slice() {
                msg!("Invalid minter configuration: minter argument doesn't match account");
                return err!(ItsError::InvalidArgument);
            }
        }
        _ => {
            // All other combinations are invalid
            msg!("Invalid minter configuration: minter field and optional accounts must be consistent");
            return err!(ItsError::InvalidArgument);
        }
    }

    // Call process_inbound_deploy directly with the context accounts
    process_inbound_deploy(
        ctx.accounts,
        token_id,
        &truncated_name,
        &truncated_symbol,
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
            .map(|account| account.key().to_bytes().to_vec()),
    });

    emit_cpi!(InterchainTokenDeployed {
        token_id,
        token_address: ctx.accounts.token_mint.key(),
        minter: ctx.accounts.minter.as_ref().map(anchor_lang::Key::key),
        name: truncated_name,
        symbol: truncated_symbol,
        decimals,
    });

    Ok(())
}

pub fn process_inbound_deploy(
    ctx: &mut ExecuteDeployInterchainToken,
    token_id: [u8; 32],
    name: &str,
    symbol: &str,
    token_manager_pda_bump: u8,
    minter_roles_pda_bump: Option<u8>,
) -> Result<()> {
    create_token_metadata(ctx, name, symbol, token_id, token_manager_pda_bump)?;

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
        let minter_roles_pda = ctx
            .minter_roles_pda
            .as_mut()
            .ok_or(ItsError::MinterRolesNotProvided)?;
        minter_roles_pda.bump =
            minter_roles_pda_bump.ok_or(ItsError::MinterRolesPdaBumpNotProvided)?;
        minter_roles_pda.roles = roles::OPERATOR | roles::FLOW_LIMITER | roles::MINTER;
    }

    Ok(())
}

fn create_token_metadata<'info>(
    accounts: &ExecuteDeployInterchainToken<'info>,
    name: &str,
    symbol: &str,
    token_id: [u8; 32],
    token_manager_bump: u8,
) -> Result<()> {
    // Create the token metadata using Metaplex CPI
    CreateV1CpiBuilder::new(&accounts.mpl_token_metadata_program.to_account_info())
        .metadata(&accounts.mpl_token_metadata_account.to_account_info())
        .token_standard(TokenStandard::Fungible)
        .mint(&accounts.token_mint.to_account_info(), false)
        .authority(&accounts.token_manager_pda.to_account_info())
        .update_authority(&accounts.token_manager_pda.to_account_info(), true)
        .payer(&accounts.payer.to_account_info())
        .is_mutable(false)
        .name(name.to_owned())
        .symbol(symbol.to_owned())
        .uri(String::with_capacity(0))
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
