use crate::{
    errors::ItsError,
    events::TokenManagerDeployed,
    instructions::get_token_metadata,
    seed_prefixes::TOKEN_MANAGER_SEED,
    state::{InterchainTokenService, TokenManager, Type},
    utils::{
        canonical_interchain_token_deploy_salt, canonical_interchain_token_id,
        interchain_token_id_internal,
    },
};
use anchor_lang::prelude::*;
use anchor_lang::solana_program::instruction::Instruction;
use anchor_lang::InstructionData;
use anchor_spl::associated_token::get_associated_token_address_with_program_id;
use anchor_spl::token_2022::spl_token_2022::extension::{
    BaseStateWithExtensions, ExtensionType, StateWithExtensions,
};
use anchor_spl::token_2022::spl_token_2022::state::Mint as SplMint;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::TokenAccount,
    token_interface::{Mint, TokenInterface},
};

#[derive(Accounts)]
#[event_cpi]
pub struct RegisterCanonicalInterchainToken<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    /// CHECK: decoded using get_token_metadata
    #[account(
        seeds = [
            b"metadata",
            mpl_token_metadata::programs::MPL_TOKEN_METADATA_ID.as_ref(),
            token_mint.key().as_ref()
        ],
        bump,
        seeds::program = mpl_token_metadata::programs::MPL_TOKEN_METADATA_ID
    )]
    pub metadata_account: AccountInfo<'info>,

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
            &canonical_interchain_token_id(&token_mint.key())
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
}

pub fn register_canonical_interchain_token_handler(
    ctx: Context<RegisterCanonicalInterchainToken>,
) -> Result<[u8; 32]> {
    msg!("Instruction: RegisterCanonicalInterchainToken");

    // Metadata is required for canonical tokens
    if let Err(_err) = get_token_metadata(
        &ctx.accounts.token_mint.to_account_info(),
        Some(&ctx.accounts.metadata_account),
    ) {
        return err!(ItsError::InvalidAccountData);
    }

    // Check if token has fee extension to determine manager type
    let token_mint_account = ctx.accounts.token_mint.to_account_info();
    let mint_data = token_mint_account.try_borrow_data()?;
    let mint = StateWithExtensions::<SplMint>::unpack(&mint_data)?;
    let has_fee_extension = mint
        .get_extension_types()?
        .contains(&ExtensionType::TransferFeeConfig);

    let token_manager_type = if has_fee_extension {
        Type::LockUnlockFee
    } else {
        Type::LockUnlock
    };

    let deploy_salt = canonical_interchain_token_deploy_salt(&ctx.accounts.token_mint.key());
    let token_id = interchain_token_id_internal(&deploy_salt);

    // Initialize the Token Manager
    TokenManager::init_account(
        &mut ctx.accounts.token_manager_pda,
        token_manager_type,
        token_id,
        ctx.accounts.token_mint.key(),
        ctx.accounts.token_manager_ata.key(),
        ctx.bumps.token_manager_pda,
    );

    emit_cpi!(TokenManagerDeployed {
        token_id,
        token_manager: *ctx.accounts.token_manager_pda.to_account_info().key,
        token_manager_type: token_manager_type.into(),
        params: None, // No additional params for canonical tokens
    });

    Ok(token_id)
}

pub fn make_register_canonical_token_instruction(
    payer: Pubkey,
    token_mint: Pubkey,
    token_program: Pubkey,
) -> (
    Instruction,
    crate::accounts::RegisterCanonicalInterchainToken,
) {
    let its_root_pda = InterchainTokenService::find_pda().0;

    let token_id = canonical_interchain_token_id(&token_mint);
    let token_manager_pda = TokenManager::find_pda(token_id, its_root_pda).0;

    let token_manager_ata = get_associated_token_address_with_program_id(
        &token_manager_pda,
        &token_mint,
        &token_program,
    );

    let (metadata_account, _) = mpl_token_metadata::accounts::Metadata::find_pda(&token_mint);
    let (event_authority, _) = Pubkey::find_program_address(&[b"__event_authority"], &crate::ID);

    let accounts = crate::accounts::RegisterCanonicalInterchainToken {
        payer,
        metadata_account,
        system_program: anchor_lang::system_program::ID,
        its_root_pda,
        token_manager_pda,
        token_mint,
        token_manager_ata,
        token_program,
        associated_token_program: anchor_spl::associated_token::spl_associated_token_account::ID,
        event_authority,
        program: crate::ID,
    };

    (
        Instruction {
            program_id: crate::ID,
            accounts: accounts.to_account_metas(None),
            data: crate::instruction::RegisterCanonicalInterchainToken {}.data(),
        },
        accounts,
    )
}
