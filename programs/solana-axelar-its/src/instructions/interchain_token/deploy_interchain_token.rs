use crate::{
    errors::ItsError,
    events::{InterchainTokenDeployed, TokenManagerDeployed},
    seed_prefixes::{INTERCHAIN_TOKEN_SEED, TOKEN_MANAGER_SEED},
    state::{roles, InterchainTokenService, TokenManager, Type, UserRoles},
    utils::{interchain_token_deployer_salt, interchain_token_id, interchain_token_id_internal},
};
use anchor_lang::solana_program::instruction::Instruction;
use anchor_lang::{prelude::*, InstructionData};
use anchor_spl::token_interface::{Mint, TokenAccount};
use anchor_spl::{
    associated_token::get_associated_token_address_with_program_id,
    associated_token::AssociatedToken,
};
use anchor_spl::{
    associated_token::spl_associated_token_account,
    token_2022::{spl_token_2022, Token2022},
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
        constraint = !its_root_pda.paused @ ItsError::Paused,
    )]
    pub its_root_pda: Account<'info, InterchainTokenService>,

    #[account(
        init,
        payer = payer,
        space = TokenManager::DISCRIMINATOR.len() + TokenManager::INIT_SPACE,
        seeds = [
            TokenManager::SEED_PREFIX,
            its_root_pda.key().as_ref(),
            &interchain_token_id(&deployer.key(), &salt),
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
        init_if_needed,
        payer = payer,
        associated_token::mint = token_mint,
        associated_token::authority = token_manager_pda,
        associated_token::token_program = token_program,
    )]
    pub token_manager_ata: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Program<'info, Token2022>,

    pub associated_token_program: Program<'info, AssociatedToken>,

    /// CHECK: sysvar address check
    #[account(address = solana_sdk_ids::sysvar::instructions::ID)]
    pub sysvar_instructions: UncheckedAccount<'info>,

    /// CHECK:
    #[account(address = mpl_token_metadata::programs::MPL_TOKEN_METADATA_ID)]
    pub mpl_token_metadata_program: UncheckedAccount<'info>,

    /// CHECK: delegated to mpl_token_metadata_program
    #[account(
        mut,
        address = mpl_token_metadata::accounts::Metadata::find_pda(&token_mint.key()).0,
    )]
    pub mpl_token_metadata_account: UncheckedAccount<'info>,

    #[account(
        init_if_needed,
        payer = payer,
        associated_token::mint = token_mint,
        associated_token::authority = deployer,
        associated_token::token_program = token_program,
    )]
    pub deployer_ata: InterfaceAccount<'info, TokenAccount>,

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
        bump,
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

    if name.len() > mpl_token_metadata::MAX_NAME_LENGTH
        || symbol.len() > mpl_token_metadata::MAX_SYMBOL_LENGTH
    {
        msg!("Name and/or symbol length too long");
        return err!(ItsError::InvalidArgument);
    }

    // Validate minter accounts and initial supply
    match (
        &ctx.accounts.minter,
        &mut ctx.accounts.minter_roles_pda,
        ctx.bumps.minter_roles_pda,
        initial_supply,
    ) {
        // Both minter accounts provided - initialize roles
        (Some(_minter), Some(minter_roles_pda), Some(bump), _supply) => {
            minter_roles_pda.bump = bump;
            minter_roles_pda.roles = roles::OPERATOR | roles::FLOW_LIMITER | roles::MINTER;
        }
        // No minter provided and zero supply
        (None, None, None, 0) => {
            msg!("Cannot deploy a zero supply token without a minter");
            return err!(ItsError::ZeroSupplyToken);
        }
        (None, None, None, _supply) => {
            // Non-zero supply with no minter = fixed supply token (valid)
        }
        // Any other case (should not occur)
        _ => {
            msg!("Invalid minter account configuration");
            return err!(ItsError::InvalidAccountData);
        }
    }

    // mint initial supply
    if initial_supply > 0 {
        mint_initial_supply(
            ctx.accounts,
            token_id,
            initial_supply,
            ctx.bumps.token_manager_pda,
        )?;
    }

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
    });

    emit_cpi!(InterchainTokenDeployed {
        token_id,
        token_address: ctx.accounts.token_mint.key(),
        minter: ctx.accounts.minter.as_ref().map(|account| *account.key),
        name,
        symbol,
        decimals,
    });

    Ok(token_id)
}

fn mint_initial_supply(
    accounts: &DeployInterchainToken<'_>,
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

    let cpi_context =
        CpiContext::new_with_signer(accounts.token_program.key(), cpi_accounts, signer_seeds);

    token_interface::mint_to(cpi_context, initial_supply)?;

    Ok(())
}

fn create_token_metadata(
    accounts: &DeployInterchainToken<'_>,
    name: String,
    symbol: String,
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
        .name(name)
        .symbol(symbol)
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

/// Creates a DeployInterchainToken instruction
pub fn make_deploy_interchain_token_instruction(
    payer: Pubkey,
    deployer: Pubkey,
    salt: [u8; 32],
    name: String,
    symbol: String,
    decimals: u8,
    initial_supply: u64,
    minter: Option<Pubkey>,
) -> (Instruction, crate::accounts::DeployInterchainToken) {
    let its_root = InterchainTokenService::find_pda().0;

    let token_id = crate::utils::interchain_token_id(&deployer, &salt);
    let token_manager_pda = crate::TokenManager::find_pda(token_id, its_root).0;
    let token_mint = crate::TokenManager::find_token_mint(token_id, its_root).0;
    let mpl_token_metadata_account = crate::TokenManager::find_token_metadata(token_id, its_root).0;
    let token_manager_ata = get_associated_token_address_with_program_id(
        &token_manager_pda,
        &token_mint,
        &spl_token_2022::ID,
    );
    let deployer_ata =
        get_associated_token_address_with_program_id(&deployer, &token_mint, &spl_token_2022::ID);
    let minter_roles_pda =
        minter.map(|minter_key| crate::UserRoles::find_pda(&token_manager_pda, &minter_key).0);

    let (event_authority, _bump) = crate::EVENT_AUTHORITY_AND_BUMP;

    let accounts = crate::accounts::DeployInterchainToken {
        payer,
        deployer,
        system_program: anchor_lang::system_program::ID,
        its_root_pda: its_root,
        token_manager_pda,
        token_mint,
        token_manager_ata,
        token_program: anchor_spl::token_2022::spl_token_2022::ID,
        associated_token_program: spl_associated_token_account::program::ID,
        sysvar_instructions: solana_sdk_ids::sysvar::instructions::ID,
        mpl_token_metadata_program: mpl_token_metadata::programs::MPL_TOKEN_METADATA_ID,
        mpl_token_metadata_account,
        deployer_ata,
        minter,
        minter_roles_pda,
        event_authority,
        program: crate::ID,
    };

    (
        Instruction {
            program_id: crate::ID,
            accounts: accounts.to_account_metas(None),
            data: crate::instruction::DeployInterchainToken {
                salt,
                name,
                symbol,
                decimals,
                initial_supply,
            }
            .data(),
        },
        accounts,
    )
}
