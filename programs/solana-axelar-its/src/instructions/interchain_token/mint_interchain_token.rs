use crate::{
    errors::ItsError,
    state::{InterchainTokenService, TokenManager, UserRoles},
};
use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

#[derive(Accounts)]
#[instruction(amount: u64)]
pub struct MintInterchainToken<'info> {
    #[account(
        mut,
        mint::token_program = token_program,
    )]
    pub mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        token::mint = mint,
        token::token_program = token_program,
    )]
    pub destination_account: InterfaceAccount<'info, TokenAccount>,

    #[account(
        seeds = [InterchainTokenService::SEED_PREFIX],
        bump = its_root_pda.bump,
        constraint = !its_root_pda.paused @ ItsError::Paused,
    )]
    pub its_root_pda: Account<'info, InterchainTokenService>,

    #[account(
        mut,
        seeds = [
            TokenManager::SEED_PREFIX,
            its_root_pda.key().as_ref(),
            &token_manager_pda.token_id,
        ],
        bump = token_manager_pda.bump,
        constraint = token_manager_pda.token_address == mint.key()
            @ ItsError::TokenMintTokenManagerMissmatch,
    )]
    pub token_manager_pda: Account<'info, TokenManager>,

    pub minter: Signer<'info>,

    #[account(
        seeds = [
            UserRoles::SEED_PREFIX,
            token_manager_pda.key().as_ref(),
            minter.key().as_ref(),
        ],
        bump = minter_roles_pda.bump,
        constraint = minter_roles_pda.has_minter_role() @ ItsError::InvalidRole,
    )]
    pub minter_roles_pda: Account<'info, UserRoles>,

    pub token_program: Interface<'info, TokenInterface>,
}

pub fn mint_interchain_token_handler(ctx: Context<MintInterchainToken>, amount: u64) -> Result<()> {
    msg!("Instruction: MintInterchainToken");

    if amount == 0 {
        return err!(ItsError::InvalidAmount);
    }

    // Mint tokens using the token manager PDA as authority
    let token_manager = &ctx.accounts.token_manager_pda;
    let its_root_pda = &ctx.accounts.its_root_pda;

    let its_root_key = its_root_pda.key();
    let token_id = &token_manager.token_id;
    let bump = token_manager.bump;

    let seeds = &[
        TokenManager::SEED_PREFIX,
        its_root_key.as_ref(),
        token_id.as_ref(),
        &[bump],
    ];
    let signer_seeds = &[&seeds[..]];

    let cpi_ctx = CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        anchor_spl::token_interface::MintTo {
            mint: ctx.accounts.mint.to_account_info(),
            to: ctx.accounts.destination_account.to_account_info(),
            authority: ctx.accounts.token_manager_pda.to_account_info(),
        },
        signer_seeds,
    );

    anchor_spl::token_interface::mint_to(cpi_ctx, amount)?;

    Ok(())
}
