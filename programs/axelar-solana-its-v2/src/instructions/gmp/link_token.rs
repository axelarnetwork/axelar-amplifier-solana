use crate::{
    errors::ItsError,
    events::TokenManagerDeployed,
    instructions::validate_mint_extensions,
    state::{token_manager, InterchainTokenService, Roles, TokenManager, UserRoles},
};
use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{Mint, TokenAccount, TokenInterface},
};

#[derive(Accounts)]
#[event_cpi]
#[instruction(
	token_id: [u8; 32],
	destination_token_address: [u8; 32],
	token_manager_type: u8,
	link_params: Vec<u8>,
)]
pub struct LinkTokenInternal<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    pub deployer: Signer<'info>,

    pub system_program: Program<'info, System>,

    #[account(
        seeds = [InterchainTokenService::SEED_PREFIX],
        bump = its_root_pda.bump,
        constraint = !its_root_pda.paused @ ItsError::Paused,
        signer, // important: only ITS can call this
    )]
    pub its_root_pda: Account<'info, InterchainTokenService>,

    #[account(
        init,
        payer = payer,
        space = TokenManager::DISCRIMINATOR.len() + TokenManager::INIT_SPACE,
        seeds = [
            TokenManager::SEED_PREFIX,
            its_root_pda.key().as_ref(),
            &token_id,
        ],
        bump,
    )]
    pub token_manager_pda: Account<'info, TokenManager>,

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

    pub operator: Option<UncheckedAccount<'info>>,

    #[account(
        init,
        payer = payer,
        space = UserRoles::DISCRIMINATOR.len() + UserRoles::INIT_SPACE,
        seeds = [
            UserRoles::SEED_PREFIX,
            token_manager_pda.key().as_ref(),
            operator.as_ref().ok_or(ItsError::OperatorNotProvided)?.key().as_ref()
        ],
        bump
    )]
    pub operator_roles_pda: Option<Account<'info, UserRoles>>,
}

pub fn link_token_internal_handler(
    ctx: Context<LinkTokenInternal>,
    token_id: [u8; 32],
    destination_token_address: [u8; 32],
    token_manager_type: u8,
    link_params: Vec<u8>,
) -> Result<()> {
    msg!("link_token_internal_handler");

    let token_manager_type: token_manager::Type = token_manager_type
        .try_into()
        .map_err(|_| ItsError::InvalidInstructionData)?;
    if token_manager::Type::NativeInterchainToken == token_manager_type {
        return err!(ItsError::InvalidInstructionData);
    }

    let token_address = Pubkey::new_from_array(
        destination_token_address
            .as_ref()
            .try_into()
            .map_err(|_err| ProgramError::InvalidAccountData)?,
    );

    let operator = match link_params.try_into() {
        Ok(operator_bytes) => Some(Pubkey::new_from_array(operator_bytes)),
        Err(_err) => None,
    };

    if operator.is_some() != ctx.accounts.operator.is_some() {
        return err!(ItsError::InvalidArgument);
    }

    if ctx.accounts.operator.is_some() != ctx.accounts.operator_roles_pda.is_some() {
        return err!(ItsError::InvalidArgument);
    }

    validate_mint_extensions(
        token_manager_type,
        &ctx.accounts.token_mint.to_account_info(),
    )?;

    TokenManager::init_account(
        &mut ctx.accounts.token_manager_pda,
        token_manager_type,
        token_id,
        token_address,
        ctx.accounts.token_manager_ata.key(),
        ctx.bumps.token_manager_pda,
    );

    if let Some(operator_roles_pda) = &mut ctx.accounts.operator_roles_pda {
        operator_roles_pda.bump = ctx
            .bumps
            .operator_roles_pda
            .ok_or(ItsError::OperatorRolesPdaNotProvided)?;
        operator_roles_pda.roles = Roles::OPERATOR | Roles::FLOW_LIMITER;
    }

    emit_cpi!(TokenManagerDeployed {
        token_id,
        token_manager: ctx.accounts.token_manager_pda.key(),
        token_manager_type: token_manager_type.into(),
        params: operator
            .map(|op| op.to_bytes().to_vec())
            .unwrap_or_default(),
    });

    Ok(())
}
