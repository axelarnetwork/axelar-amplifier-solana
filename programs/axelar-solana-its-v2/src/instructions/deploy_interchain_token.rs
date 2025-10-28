use crate::{
    errors::ItsError,
    events::{InterchainTokenDeployed, InterchainTokenIdClaimed, TokenManagerDeployed},
    seed_prefixes::{INTERCHAIN_TOKEN_SEED, TOKEN_MANAGER_SEED},
    state::{token_manager, InterchainTokenService, Roles, TokenManager, Type, UserRoles},
    utils::{interchain_token_deployer_salt, interchain_token_id, interchain_token_id_internal},
};
use anchor_lang::prelude::*;
use anchor_spl::token_2022::spl_token_2022::extension::BaseStateWithExtensions;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_2022::spl_token_2022::{extension::StateWithExtensions, state::Mint as SplMint},
};
use anchor_spl::{
    token_2022::spl_token_2022::extension::ExtensionType,
    token_interface::{Mint, TokenAccount, TokenInterface},
};
use mpl_token_metadata::{instructions::CreateV1CpiBuilder, types::TokenStandard};

#[derive(Accounts)]
#[event_cpi]
#[instruction(salt: [u8; 32], name: String, symbol: String, decimals: u8, initial_supply: u64)]
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
        constraint = !its_root_pda.paused @ ItsError::Paused
    )]
    pub its_root_pda: Account<'info, InterchainTokenService>,

    /// Token Manager PDA for this token
    #[account(
        init,
        payer = payer,
        space = TokenManager::DISCRIMINATOR.len() + TokenManager::INIT_SPACE,
        seeds = [
            TOKEN_MANAGER_SEED,
            its_root_pda.key().as_ref(),
            &interchain_token_id(&deployer.key(), &salt)
        ],
        bump
    )]
    pub token_manager_pda: Account<'info, TokenManager>,

    /// The mint account for the new token (Token-2022 compatible)
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
            &interchain_token_id(&deployer.key(), &salt)
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

    /// Sysvar for instructions (used by Metaplex)
    #[account(address = anchor_lang::solana_program::sysvar::instructions::id())]
    pub sysvar_instructions: UncheckedAccount<'info>,

    /// Metaplex Token Metadata program
    #[account(address = mpl_token_metadata::programs::MPL_TOKEN_METADATA_ID)]
    pub mpl_token_metadata_program: UncheckedAccount<'info>,

    /// CHECK: delegated to mpl_token_metadata_program
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
    pub mpl_token_metadata_account: AccountInfo<'info>,

    /// Deployer’s associated token account
    #[account(
        init,
        payer = payer,
        associated_token::mint = token_mint,
        associated_token::authority = deployer,
        associated_token::token_program = token_program
    )]
    pub deployer_ata: InterfaceAccount<'info, TokenAccount>,

    // Minter accounts
    pub minter: Option<UncheckedAccount<'info>>,

    #[account(
        init,
        payer = payer,
        space = UserRoles::DISCRIMINATOR.len() + UserRoles::INIT_SPACE,
        constraint = minter.is_some(),
        seeds = [
            UserRoles::SEED_PREFIX,
            token_manager_pda.key().as_ref(),
            minter.as_ref().ok_or(ItsError::MinterNotProvided)?.key().as_ref()
        ],
        bump
    )]
    pub minter_roles_pda: Option<Account<'info, UserRoles>>,
}

pub fn deploy_interchain_token_handler(
    ctx: Context<DeployInterchainToken>,
    salt: [u8; 32],
    name: String,
    symbol: String,
    decimals: u8,
    initial_supply: u64,
) -> Result<[u8; 32]> {
    let deploy_salt = interchain_token_deployer_salt(ctx.accounts.deployer.key, &salt);
    let token_id = interchain_token_id_internal(&deploy_salt);

    if initial_supply == 0
        && (ctx.accounts.minter.is_none() || ctx.accounts.minter_roles_pda.is_none())
    {
        return err!(ItsError::ZeroSupplyToken);
    }

    if name.len() > mpl_token_metadata::MAX_NAME_LENGTH
        || symbol.len() > mpl_token_metadata::MAX_SYMBOL_LENGTH
    {
        msg!("Name and/or symbol length too long");
        return err!(ItsError::InvalidArgument);
    }

    emit_cpi!(InterchainTokenIdClaimed {
        token_id,
        deployer: *ctx.accounts.deployer.key,
        salt: deploy_salt,
    });

    // process_inbound_deploy

    // setup_mint
    if initial_supply > 0 {
        mint_initial_supply(
            ctx.accounts,
            token_id,
            initial_supply,
            ctx.bumps.token_manager_pda,
        )?;
    }

    // setup_metadata
    create_token_metadata(
        ctx.accounts,
        name.clone(),
        symbol.clone(),
        token_id,
        ctx.bumps.token_manager_pda,
    )?;

    TokenManager::init_account(
        &mut ctx.accounts.token_manager_pda,
        Type::NativeInterchainToken,
        token_id,
        ctx.accounts.token_mint.key(),
        ctx.accounts.token_manager_ata.key(),
        ctx.bumps.token_manager_pda,
    );

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

    // Initialize UserRoles
    if let (Some(minter_roles_pda), Some(bump)) = (
        ctx.accounts.minter_roles_pda.as_mut(),
        ctx.bumps.minter_roles_pda,
    ) {
        minter_roles_pda.bump = bump;
        minter_roles_pda.roles = Roles::OPERATOR | Roles::FLOW_LIMITER | Roles::MINTER;
    }

    emit_cpi!(InterchainTokenDeployed {
        token_id,
        token_address: ctx.accounts.token_mint.key(),
        minter: ctx
            .accounts
            .minter
            .as_ref()
            .map(|account| *account.key)
            .unwrap_or_default(),
        name,
        symbol,
        decimals,
    });

    Ok(token_id)
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
    name: String,
    symbol: String,
    token_id: [u8; 32],
    token_manager_bump: u8,
) -> Result<()> {
    // NOTE: truncate panics if MAX_LENTH
    // does not lie on a char boundary.
    // TODO should we handle it gracefully?

    let mut truncated_name = name;
    let mut truncated_symbol = symbol;
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

// TODO: deprecate this, replace with Type::assert_supports_mint_extensions
pub fn validate_mint_extensions(
    ty: token_manager::Type,
    token_mint: &AccountInfo<'_>,
) -> Result<()> {
    let mint_data = token_mint.try_borrow_data()?;
    let mint = StateWithExtensions::<SplMint>::unpack(&mint_data)?;

    if matches!(
        (
            ty,
            mint.get_extension_types()?
                .contains(&ExtensionType::TransferFeeConfig)
        ),
        (token_manager::Type::LockUnlock, true) | (token_manager::Type::LockUnlockFee, false)
    ) {
        msg!("The mint extension is not compatible with the TokenManager type");
        return err!(ItsError::InvalidInstructionData);
    }

    Ok(())
}
