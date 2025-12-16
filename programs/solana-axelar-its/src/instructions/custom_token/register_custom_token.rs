use crate::{
    errors::ItsError,
    events::TokenManagerDeployed,
    instructions::validate_mint_extensions,
    seed_prefixes::TOKEN_MANAGER_SEED,
    state::{roles, token_manager::Type, InterchainTokenService, TokenManager, UserRoles},
    utils::{interchain_token_id_internal, linked_token_deployer_salt},
};
use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{Mint, TokenAccount, TokenInterface},
};

#[derive(Accounts)]
#[event_cpi]
#[instruction(salt: [u8; 32], token_manager_type: Type, operator: Option<Pubkey>)]
pub struct RegisterCustomToken<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    pub deployer: Signer<'info>,

    pub system_program: Program<'info, System>,

    #[account(
        seeds = [InterchainTokenService::SEED_PREFIX],
        bump = its_root_pda.bump,
        constraint = !its_root_pda.paused @ ItsError::Paused
    )]
    pub its_root_pda: Account<'info, InterchainTokenService>,

    #[account(
        init,
        payer = payer,
        space = TokenManager::DISCRIMINATOR.len() + TokenManager::INIT_SPACE,
        seeds = [
            TOKEN_MANAGER_SEED,
            its_root_pda.key().as_ref(),
            &interchain_token_id_internal(&linked_token_deployer_salt(&deployer.key(), &salt))
        ],
        bump
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

    /// CHECK:
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

pub fn register_custom_token_handler(
    ctx: Context<RegisterCustomToken>,
    salt: [u8; 32],
    token_manager_type: Type,
    operator: Option<Pubkey>,
) -> Result<[u8; 32]> {
    msg!("Instruction: RegisterCustomToken");

    // Validate that the token manager type is not NativeInterchainToken
    if token_manager_type == Type::NativeInterchainToken {
        return err!(ItsError::InvalidInstructionData);
    }

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

    let deploy_salt = linked_token_deployer_salt(&ctx.accounts.deployer.key(), &salt);
    let token_id = interchain_token_id_internal(&deploy_salt);

    validate_mint_extensions(
        token_manager_type,
        &ctx.accounts.token_mint.to_account_info(),
    )?;

    // Initialize the token manager
    TokenManager::init_account(
        &mut ctx.accounts.token_manager_pda,
        token_manager_type,
        token_id,
        ctx.accounts.token_mint.key(),
        ctx.accounts.token_manager_ata.key(),
        ctx.bumps.token_manager_pda,
    );

    // Initialize operator roles if provided
    if let (Some(operator_roles_pda), Some(bump)) = (
        ctx.accounts.operator_roles_pda.as_mut(),
        ctx.bumps.operator_roles_pda,
    ) {
        operator_roles_pda.bump = bump;
        operator_roles_pda.roles = roles::OPERATOR | roles::FLOW_LIMITER;
    }

    // Emit TokenManagerDeployed event
    emit_cpi!(TokenManagerDeployed {
        token_id,
        token_manager: ctx.accounts.token_manager_pda.key(),
        token_manager_type: token_manager_type.into(),
        params: operator.map(|op| op.to_bytes().to_vec()),
    });

    Ok(token_id)
}
