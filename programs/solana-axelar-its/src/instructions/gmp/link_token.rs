use crate::{
    errors::ItsError,
    events::TokenManagerDeployed,
    instructions::validate_mint_extensions,
    state::{token_manager, InterchainTokenService, roles, TokenManager, UserRoles},
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
pub struct ExecuteLinkToken<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

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

    #[account(mint::token_program = token_program)]
    pub token_mint: InterfaceAccount<'info, Mint>,

    #[account(
        init_if_needed,
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

pub fn execute_link_token_handler(
    ctx: Context<ExecuteLinkToken>,
    token_id: [u8; 32],
    destination_token_address: [u8; 32],
    token_manager_type: u8,
    link_params: Vec<u8>,
) -> Result<()> {
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
            .map_err(|_err| ItsError::InvalidAccountData)?,
    );

    // verify that the provided token address matches the mint address
    require_keys_eq!(
        token_address,
        ctx.accounts.token_mint.key(),
        ItsError::InvalidTokenMint,
    );

    let operator = match link_params.try_into() {
        Ok(operator_bytes) => Some(Pubkey::new_from_array(operator_bytes)),
        Err(_err) => None,
    };

    // check that all operator-related accounts are provided or none at all
    // and that the provided operator matches the operator account
    match (
        operator,
        ctx.accounts.operator.as_ref(),
        ctx.accounts.operator_roles_pda.as_ref(),
    ) {
        (Some(operator_pubkey), Some(operator_account), Some(_roles_pda)) => {
            if operator_pubkey != operator_account.key() {
                return err!(ItsError::InvalidArgument);
            }
        }
        (None, None, None) => {}
        _ => return err!(ItsError::InvalidArgument),
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
        operator_roles_pda.roles = roles::OPERATOR | roles::FLOW_LIMITER;
    }

    emit_cpi!(TokenManagerDeployed {
        token_id,
        token_manager: ctx.accounts.token_manager_pda.key(),
        token_manager_type: token_manager_type.into(),
        params: operator.map(|op| op.to_bytes().to_vec()),
    });

    Ok(())
}
