use crate::{
    errors::ITSError,
    events::{InterchainTokenDeployed, InterchainTokenIdClaimed, TokenManagerDeployed},
    seed_prefixes::{INTERCHAIN_TOKEN_SEED, TOKEN_MANAGER_SEED},
    state::{
        token_manager, FlowState, InterchainTokenService, Roles, TokenManager, Type, UserRoles,
    },
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
    #[account(mut)]
    pub payer: Signer<'info>,

    pub deployer: Signer<'info>,

    pub system_program: Program<'info, System>,

    #[account(
        seeds = [InterchainTokenService::SEED_PREFIX],
        bump = its_root_pda.bump,
        constraint = !its_root_pda.paused @ ITSError::Paused
    )]
    pub its_root_pda: Account<'info, InterchainTokenService>,

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

    #[account(
        init,
        payer = payer,
        associated_token::mint = token_mint,
        associated_token::authority = token_manager_pda,
        associated_token::token_program = token_program
    )]
    pub token_manager_ata: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,

    pub associated_token_program: Program<'info, AssociatedToken>,

    pub rent: Sysvar<'info, Rent>,

    #[account(address = anchor_lang::solana_program::sysvar::instructions::id())]
    pub sysvar_instructions: UncheckedAccount<'info>,

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

pub fn deploy_interchain_token_handler(
    ctx: Context<DeployInterchainToken>,
    salt: [u8; 32],
    name: String,
    symbol: String,
    decimals: u8,
    initial_supply: u64,
) -> Result<()> {
    let deploy_salt = interchain_token_deployer_salt(ctx.accounts.deployer.key, &salt);
    let token_id = interchain_token_id_internal(&deploy_salt);

    if initial_supply == 0
        && (ctx.accounts.minter.is_none() || ctx.accounts.minter_roles_pda.is_none())
    {
        return err!(ITSError::InvalidArgument);
    }

    if name.len() > mpl_token_metadata::MAX_NAME_LENGTH
        || symbol.len() > mpl_token_metadata::MAX_SYMBOL_LENGTH
    {
        msg!("Name and/or symbol length too long");
        return err!(ITSError::InvalidArgument);
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
            &ctx.accounts,
            token_id,
            initial_supply,
            ctx.bumps.token_manager_pda,
        )?;
    }

    // setup_metadata
    create_token_metadata(
        &ctx.accounts,
        name.clone(),
        symbol.clone(),
        token_id,
        ctx.bumps.token_manager_pda,
    )?;

    // super::token_manager::deploy(...)
    validate_mint_extensions(
        Type::NativeInterchainToken,
        &ctx.accounts.token_mint.to_account_info(),
    )?;

    initialize_token_manager(
        &mut ctx.accounts.token_manager_pda,
        token_id,
        ctx.accounts.token_mint.key(),
        ctx.accounts.token_manager_ata.key(),
        ctx.bumps.token_manager_pda,
        Type::NativeInterchainToken,
    )?;

    emit_cpi!(TokenManagerDeployed {
        token_id,
        token_manager: *ctx.accounts.token_manager_pda.to_account_info().key,
        token_manager_type: Type::NativeInterchainToken.into(),
        params: ctx
            .accounts
            .minter
            .as_ref()
            .map(|account| account.key().to_bytes().to_vec())
            .unwrap_or_default(),
    });

    // Initialize UserRoles
    if ctx.accounts.minter.is_some() && ctx.accounts.minter_roles_pda.is_some() {
        let minter_roles_pda = &mut ctx.accounts.minter_roles_pda.as_mut().unwrap();
        minter_roles_pda.bump = ctx.bumps.minter_roles_pda.unwrap();
        minter_roles_pda.roles = Roles::OPERATOR | Roles::FLOW_LIMITER | Roles::MINTER;
    }

    emit_cpi!(InterchainTokenDeployed {
        token_id,
        token_address: *ctx.accounts.token_mint.to_account_info().key,
        minter: ctx
            .accounts
            .minter
            .clone()
            .map(|account| *account.key)
            .unwrap_or_default(),
        name: name.clone(),
        symbol: symbol.clone(),
        decimals: decimals,
    });

    anchor_lang::solana_program::program::set_return_data(&token_id);

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
    name: String,
    symbol: String,
    token_id: [u8; 32],
    token_manager_bump: u8,
) -> Result<()> {
    // Truncate name and symbol to fit Metaplex limits
    let truncated_name = if name.len() > mpl_token_metadata::MAX_NAME_LENGTH {
        name[..mpl_token_metadata::MAX_NAME_LENGTH].to_string()
    } else {
        name.clone()
    };

    let truncated_symbol = if symbol.len() > mpl_token_metadata::MAX_SYMBOL_LENGTH {
        symbol[..mpl_token_metadata::MAX_SYMBOL_LENGTH].to_string()
    } else {
        symbol.clone()
    };

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
        msg!("The mint is not compatible with the type");
        return err!(ITSError::InvalidInstructionData);
    }

    Ok(())
}

pub fn initialize_token_manager(
    token_manager_pda: &mut Account<TokenManager>,
    token_id: [u8; 32],
    token_address: Pubkey,
    associated_token_account: Pubkey,
    bump: u8,
    token_manager_type: Type,
) -> Result<()> {
    token_manager_pda.ty = token_manager_type;
    token_manager_pda.token_id = token_id;
    token_manager_pda.token_address = token_address;
    token_manager_pda.associated_token_account = associated_token_account;
    token_manager_pda.flow_slot = FlowState::new(None, 0);
    token_manager_pda.bump = bump;

    Ok(())
}
