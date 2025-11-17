use crate::{
    state::{InterchainTokenService, Roles, TokenManager, Type, UserRoles},
    ItsError,
};
use anchor_lang::prelude::*;
use anchor_lang::solana_program::program_option::COption;
use anchor_spl::{
    token_2022::spl_token_2022::{
        extension::StateWithExtensions,
        instruction::{set_authority, AuthorityType},
        state::Mint,
    },
    token_interface::TokenInterface,
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
        constraint = mint.key() == token_manager.token_address @ ItsError::TokenMintTokenManagerMissmatch,
    )]
    pub mint: UncheckedAccount<'info>,

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
        constraint = token_manager.ty == Type::MintBurn || token_manager.ty == Type::MintBurnFrom
            @ ItsError::InvalidTokenManagerType,
    )]
    pub token_manager: Account<'info, TokenManager>,

    /// User roles account for the payer (will receive MINTER role)
    #[account(
        init_if_needed,
        payer = payer,
        space = UserRoles::DISCRIMINATOR.len() + UserRoles::INIT_SPACE,
        seeds = [
            UserRoles::SEED_PREFIX,
            token_manager.key().as_ref(),
            payer.key().as_ref(),
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
    let payer = &ctx.accounts.payer;
    let minter_roles = &mut ctx.accounts.minter_roles;
    let token_program = &ctx.accounts.token_program;

    // Validate that token_program owns the mint account
    if token_program.key() != *mint.owner {
        return err!(ItsError::InvalidTokenManagerType);
    }

    // Check the current mint authority
    let maybe_mint_authority = {
        let mint_data = mint.try_borrow_data()?;
        let mint_state = StateWithExtensions::<Mint>::unpack(&mint_data)?;
        mint_state.base.mint_authority
    };

    match maybe_mint_authority {
        COption::None => {
            msg!("Cannot hand over mint authority of a TokenManager for non-mintable token");
            return err!(ItsError::InvalidArgument);
        }
        COption::Some(mint_authority) if mint_authority == authority.key() => {
            // The given authority is the mint authority. Transfer it to the TokenManager

            let authority_transfer_ix = set_authority(
                token_program.key,
                mint.key,
                Some(token_manager.to_account_info().key),
                AuthorityType::MintTokens,
                authority.key,
                &[],
            )?;

            anchor_lang::solana_program::program::invoke(
                &authority_transfer_ix,
                &[mint.to_account_info(), authority.to_account_info()],
            )?;

            // Setup minter role for the payer
            minter_roles.roles.insert(Roles::MINTER);
            minter_roles.bump = ctx.bumps.minter_roles;

            msg!(
                "Transferred mint authority to token manager and granted MINTER role to {}",
                payer.key()
            );

            Ok(())
        }
        COption::Some(_) => {
            msg!("Signer is not the mint authority");
            err!(ItsError::InvalidArgument)
        }
    }
}
