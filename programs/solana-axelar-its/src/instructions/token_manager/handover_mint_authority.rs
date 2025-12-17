use crate::{
    roles,
    state::{InterchainTokenService, TokenManager, Type, UserRoles},
    ItsError,
};
use anchor_lang::prelude::*;
use anchor_spl::{
    token_2022::spl_token_2022::instruction::AuthorityType,
    token_interface::{Mint, TokenInterface},
};

#[derive(Accounts)]
#[instruction(token_id: [u8; 32])]
pub struct HandoverMintAuthority<'info> {
    /// Payer for transaction fees and account creation
    #[account(mut)]
    pub payer: Signer<'info>,

    /// The current mint authority of the token (must be a signer)
    pub authority: Signer<'info>,

    /// The token mint account
    #[account(
        mut,
        mint::authority = authority,
        mint::token_program = token_program,
        constraint = mint.key() == token_manager.token_address
            @ ItsError::TokenMintTokenManagerMissmatch,
    )]
    pub mint: InterfaceAccount<'info, Mint>,

    /// The ITS root PDA
    #[account(
        seeds = [InterchainTokenService::SEED_PREFIX],
        bump = its_root.bump,
    )]
    pub its_root: Account<'info, InterchainTokenService>,

    /// The TokenManager account
    #[account(
        seeds = [
            TokenManager::SEED_PREFIX,
            its_root.key().as_ref(),
            &token_id,
        ],
        bump = token_manager.bump,
        // Ensure the token manager type is mintable
        constraint = token_manager.ty == Type::MintBurn
            || token_manager.ty == Type::MintBurnFrom
            @ ItsError::InvalidTokenManagerType,
    )]
    pub token_manager: Account<'info, TokenManager>,

    /// User roles account for the authority (will receive MINTER role)
    #[account(
        init_if_needed,
        payer = payer,
        space = UserRoles::DISCRIMINATOR.len() + UserRoles::INIT_SPACE,
        seeds = [
            UserRoles::SEED_PREFIX,
            token_manager.key().as_ref(),
            authority.key().as_ref(),
        ],
        bump,
    )]
    pub minter_roles: Account<'info, UserRoles>,

    pub token_program: Interface<'info, TokenInterface>,

    pub system_program: Program<'info, System>,
}

pub fn handover_mint_authority_handler(
    ctx: Context<HandoverMintAuthority>,
    _token_id: [u8; 32],
) -> Result<()> {
    msg!("Instruction: HandoverMintAuthority");

    let mint = &ctx.accounts.mint;
    let authority = &ctx.accounts.authority;
    let token_manager = &ctx.accounts.token_manager;
    let minter_roles = &mut ctx.accounts.minter_roles;

    // The given authority is the mint authority. Transfer it to the TokenManager

    let cpi_context = CpiContext::new(
        ctx.accounts.token_program.key(),
        anchor_spl::token_interface::SetAuthority {
            current_authority: authority.to_account_info(),
            account_or_mint: mint.to_account_info(),
        },
    );

    anchor_spl::token_interface::set_authority(
        cpi_context,
        AuthorityType::MintTokens,
        Some(token_manager.key()),
    )?;

    // Setup minter role for the payer
    minter_roles.insert(roles::MINTER);
    minter_roles.bump = ctx.bumps.minter_roles;

    msg!(
        "Transferred mint authority to token manager and granted MINTER role to {}",
        authority.key()
    );

    Ok(())
}
