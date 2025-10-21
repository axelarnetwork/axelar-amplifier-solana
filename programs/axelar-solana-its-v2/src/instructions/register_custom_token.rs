use crate::{
    errors::ITSError,
    events::{InterchainTokenIdClaimed, TokenManagerDeployed},
    instructions::{initialize_token_manager, validate_mint_extensions},
    seed_prefixes::TOKEN_MANAGER_SEED,
    state::{token_manager::Type, InterchainTokenService, Roles, TokenManager, UserRoles},
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
            &interchain_token_id_internal(&linked_token_deployer_salt(&deployer.key(), &salt))
        ],
        bump
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

    pub rent: Sysvar<'info, Rent>,

    pub operator: Option<UncheckedAccount<'info>>,

    #[account(
        init,
        payer = payer,
        space = UserRoles::DISCRIMINATOR.len() + UserRoles::INIT_SPACE,
        seeds = [
            UserRoles::SEED_PREFIX,
            token_manager_pda.key().as_ref(),
            operator.as_ref().unwrap().key().as_ref()
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
) -> Result<()> {
    msg!("Instruction: RegisterCustomToken");

    // Validate that the token manager type is not NativeInterchainToken
    if token_manager_type == Type::NativeInterchainToken {
        return err!(ITSError::InvalidInstructionData);
    }

    // Validate operator consistency
    if operator.is_some() != ctx.accounts.operator.is_some() {
        return err!(ITSError::InvalidArgument);
    }

    if ctx.accounts.operator.is_some() != ctx.accounts.operator_roles_pda.is_some() {
        return err!(ITSError::InvalidArgument);
    }

    let deploy_salt = linked_token_deployer_salt(&ctx.accounts.deployer.key(), &salt);
    let token_id = interchain_token_id_internal(&deploy_salt);

    // Emit InterchainTokenIdClaimed event
    emit_cpi!(InterchainTokenIdClaimed {
        token_id,
        deployer: ctx.accounts.payer.key(),
        salt: deploy_salt,
    });

    // Not needed for custom tokens
    validate_mint_extensions(
        token_manager_type,
        &ctx.accounts.token_mint.to_account_info(),
    )?;

    // Initialize the token manager
    initialize_token_manager(
        &mut ctx.accounts.token_manager_pda,
        token_id,
        ctx.accounts.token_mint.key(),
        ctx.accounts.token_manager_ata.key(),
        ctx.bumps.token_manager_pda,
        token_manager_type,
    )?;

    // Initialize operator roles if provided
    if let Some(operator_roles_pda) = &mut ctx.accounts.operator_roles_pda {
        operator_roles_pda.bump = ctx.bumps.operator_roles_pda.unwrap();
        operator_roles_pda.roles = Roles::OPERATOR | Roles::FLOW_LIMITER;
    }

    // Emit TokenManagerDeployed event
    emit_cpi!(TokenManagerDeployed {
        token_id,
        token_manager: ctx.accounts.token_manager_pda.key(),
        token_manager_type: token_manager_type.into(),
        params: operator
            .map(|op| op.to_bytes().to_vec())
            .unwrap_or_default(),
    });

    // Set return data with the token_id
    anchor_lang::solana_program::program::set_return_data(&token_id);

    Ok(())
}
