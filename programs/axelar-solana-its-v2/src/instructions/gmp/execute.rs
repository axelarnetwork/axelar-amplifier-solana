use crate::{errors::ITSError, state::InterchainTokenService};
use anchor_lang::{prelude::*, InstructionData};
use anchor_spl::{associated_token::AssociatedToken, token_interface::TokenInterface};
use axelar_solana_gateway_v2::{executable_accounts, Message};
use interchain_token_transfer_gmp::GMPPayload;
use solana_program::instruction::Instruction;

executable_accounts!();

#[derive(Accounts)]
#[event_cpi]
pub struct Execute<'info> {
    // GMP Accounts
    pub executable: AxelarExecuteAccounts<'info>,
    // ITS Accounts
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        seeds = [InterchainTokenService::SEED_PREFIX],
        bump = its_root_pda.bump,
        constraint = !its_root_pda.paused @ ITSError::Paused
    )]
    pub its_root_pda: Account<'info, InterchainTokenService>,

    #[account(mut)]
    pub token_manager_pda: UncheckedAccount<'info>,

    #[account(mut)]
    pub token_mint: UncheckedAccount<'info>,

    #[account(mut)]
    pub token_manager_ata: UncheckedAccount<'info>,

    #[account(address = anchor_spl::token_2022::ID)]
    pub token_program: Interface<'info, TokenInterface>,

    #[account(address = anchor_spl::associated_token::ID)]
    pub associated_token_program: Program<'info, AssociatedToken>,

    pub system_program: Program<'info, System>,

    /// CHECK: Rent sysvar
    #[account(address = anchor_lang::solana_program::sysvar::rent::ID)]
    pub rent: Sysvar<'info, Rent>,
    // Remaining accounts
    #[account(mut)]
    pub deployer_ata: UncheckedAccount<'info>,
    #[account(mut)]
    pub minter: Option<UncheckedAccount<'info>>,
    #[account(mut)]
    pub minter_roles_pda: Option<UncheckedAccount<'info>>,
    #[account(mut)]
    pub mpl_token_metadata_account: UncheckedAccount<'info>,
    pub mpl_token_metadata_program: UncheckedAccount<'info>,
    pub sysvar_instructions: UncheckedAccount<'info>,
}

pub fn execute_handler(ctx: Context<Execute>, message: Message, payload: Vec<u8>) -> Result<()> {
    validate_message(&ctx.accounts.executable, message.clone(), &payload)?;

    msg!("execute_handler");
    // ITS specific logic

    if message.source_address != ctx.accounts.its_root_pda.its_hub_address {
        msg!("Untrusted source address: {}", message.source_address);
        return err!(ITSError::InvalidInstructionData);
    }

    let GMPPayload::ReceiveFromHub(inner) =
        GMPPayload::decode(&payload).map_err(|_err| ProgramError::InvalidInstructionData)?
    else {
        msg!("Unsupported GMP payload");
        return err!(ITSError::InvalidInstructionData);
    };

    if !ctx
        .accounts
        .its_root_pda
        .is_trusted_chain(&inner.source_chain)
    {
        msg!("Untrusted source chain: {}", inner.source_chain);
        return err!(ITSError::InvalidInstructionData);
    }

    let payload =
        GMPPayload::decode(&inner.payload).map_err(|_err| ProgramError::InvalidInstructionData)?;

    perform_self_cpi(payload, ctx)?;

    Ok(())
}

fn perform_self_cpi(payload: GMPPayload, ctx: Context<Execute>) -> Result<()> {
    match payload {
        GMPPayload::InterchainTransfer(transfer) => interchain_transfer_self_invoke(ctx, transfer),
        GMPPayload::DeployInterchainToken(deploy) => {
            deploy_interchain_token_self_invoke(ctx, deploy)
        }
        GMPPayload::LinkToken(payload) => link_token_self_invoke(ctx, payload),
        GMPPayload::SendToHub(_)
        | GMPPayload::ReceiveFromHub(_)
        | GMPPayload::RegisterTokenMetadata(_) => err!(ITSError::InvalidInstructionData),
    }
}

fn interchain_transfer_self_invoke(
    ctx: Context<Execute>,
    transfer: interchain_token_transfer_gmp::InterchainTransfer,
) -> Result<()> {
    Ok(())
}

fn link_token_self_invoke(
    ctx: Context<Execute>,
    payload: interchain_token_transfer_gmp::LinkToken,
) -> Result<()> {
    let token_id = payload.token_id.0;

    let destination_token_address: [u8; 32] = payload
        .destination_token_address
        .as_ref()
        .try_into()
        .map_err(|_| ProgramError::InvalidAccountData)?;

    let token_manager_type: u8 = payload
        .token_manager_type
        .try_into()
        .map_err(|_| ProgramError::ArithmeticOverflow)?; // U256 to u8

    let link_params = payload.link_params.to_vec(); // Vec<u8>

    // Create the instruction data using Anchor's InstructionData trait
    let instruction_data = crate::instruction::LinkTokenInternal {
        token_id,
        destination_token_address,
        token_manager_type,
        link_params: link_params.clone(),
    };

    // Build the accounts using Anchor's generated accounts struct
    let accounts = crate::accounts::LinkTokenInternal {
        payer: ctx.accounts.payer.key(),
        deployer: ctx.accounts.payer.key(), // Use payer as deployer for GMP
        system_program: ctx.accounts.system_program.key(),
        its_root_pda: ctx.accounts.its_root_pda.key(),
        token_manager_pda: ctx.accounts.token_manager_pda.key(),
        token_mint: ctx.accounts.token_mint.key(),
        token_manager_ata: ctx.accounts.token_manager_ata.key(),
        token_program: ctx.accounts.token_program.key(),
        associated_token_program: ctx.accounts.associated_token_program.key(),
        rent: ctx.accounts.rent.key(),
        operator: ctx.accounts.minter.as_ref().map(|acc| acc.key()), // Use minter as operator
        operator_roles_pda: ctx.accounts.minter_roles_pda.as_ref().map(|acc| acc.key()),
        // for event cpi
        event_authority: ctx.accounts.event_authority.key(),
        program: ctx.accounts.program.key(),
    };

    // Create the instruction
    let link_instruction = Instruction {
        program_id: crate::id(),
        accounts: accounts.to_account_metas(None),
        data: instruction_data.data(),
    };

    // Collect all account infos (same pattern as deploy_interchain_token_self_invoke)
    let account_infos = vec![
        ctx.accounts.payer.to_account_info(),
        ctx.accounts.payer.to_account_info(), // deployer (same as payer)
        ctx.accounts.system_program.to_account_info(),
        ctx.accounts.its_root_pda.to_account_info(),
        ctx.accounts.token_manager_pda.to_account_info(),
        ctx.accounts.token_mint.to_account_info(),
        ctx.accounts.token_manager_ata.to_account_info(),
        ctx.accounts.token_program.to_account_info(),
        ctx.accounts.associated_token_program.to_account_info(),
        ctx.accounts.rent.to_account_info(),
        // Optional operator: use actual account if exists, else use program ID
        ctx.accounts
            .minter
            .as_ref()
            .map(|acc| acc.to_account_info())
            .unwrap_or(ctx.accounts.program.to_account_info()),
        // Optional operator_roles_pda: use actual account if exists, else use program ID
        ctx.accounts
            .minter_roles_pda
            .as_ref()
            .map(|acc| acc.to_account_info())
            .unwrap_or(ctx.accounts.program.to_account_info()),
        // Event CPI accounts
        ctx.accounts.event_authority.to_account_info(),
        ctx.accounts.program.to_account_info(),
    ];

    invoke_signed_with_its_root_pda(
        &link_instruction,
        &account_infos,
        ctx.accounts.its_root_pda.bump,
    )
}

fn deploy_interchain_token_self_invoke(
    ctx: Context<Execute>,
    deploy: interchain_token_transfer_gmp::DeployInterchainToken,
) -> Result<()> {
    // Extract data from the deploy payload
    let token_id = deploy.token_id.0;
    let name = deploy.name;
    let symbol = deploy.symbol;
    let decimals = deploy.decimals;

    // Create the instruction data using Anchor's InstructionData trait
    let instruction_data = crate::instruction::DeployInterchainTokenInternal {
        token_id,
        name: name.clone(),
        symbol: symbol.clone(),
        decimals,
    };

    // Build the accounts using Anchor's generated accounts struct
    let accounts = crate::accounts::DeployInterchainTokenInternal {
        payer: ctx.accounts.payer.key(),
        deployer: ctx.accounts.payer.key(), // todo: do we need to pass a separate deployer?
        system_program: ctx.accounts.system_program.key(),
        its_root_pda: ctx.accounts.its_root_pda.key(),
        token_manager_pda: ctx.accounts.token_manager_pda.key(),
        token_mint: ctx.accounts.token_mint.key(),
        token_manager_ata: ctx.accounts.token_manager_ata.key(),
        token_program: ctx.accounts.token_program.key(),
        associated_token_program: ctx.accounts.associated_token_program.key(),
        rent: ctx.accounts.rent.key(),
        sysvar_instructions: ctx.accounts.sysvar_instructions.key(),
        mpl_token_metadata_program: ctx.accounts.mpl_token_metadata_program.key(),
        mpl_token_metadata_account: ctx.accounts.mpl_token_metadata_account.key(),
        deployer_ata: ctx.accounts.deployer_ata.key(),
        minter: ctx.accounts.minter.as_ref().map(|acc| acc.key()),
        minter_roles_pda: ctx.accounts.minter_roles_pda.as_ref().map(|acc| acc.key()),
        // for event cpi
        event_authority: ctx.accounts.event_authority.key(),
        program: ctx.accounts.program.key(),
    };

    // Create the instruction
    let deploy_instruction = Instruction {
        program_id: crate::id(),
        accounts: accounts.to_account_metas(None),
        data: instruction_data.data(),
    };

    let account_infos = vec![
        ctx.accounts.payer.to_account_info(),
        ctx.accounts.payer.to_account_info(), // todo: do we need to pass a separate deployer?
        ctx.accounts.system_program.to_account_info(),
        ctx.accounts.its_root_pda.to_account_info(),
        ctx.accounts.token_manager_pda.to_account_info(),
        ctx.accounts.token_mint.to_account_info(),
        ctx.accounts.token_manager_ata.to_account_info(),
        ctx.accounts.token_program.to_account_info(),
        ctx.accounts.associated_token_program.to_account_info(),
        ctx.accounts.rent.to_account_info(),
        ctx.accounts.sysvar_instructions.to_account_info(),
        ctx.accounts.mpl_token_metadata_program.to_account_info(),
        ctx.accounts.mpl_token_metadata_account.to_account_info(),
        ctx.accounts.deployer_ata.to_account_info(),
        // Optional minter: use actual account if exists, else use program ID
        ctx.accounts
            .minter
            .as_ref()
            .map(|acc| acc.to_account_info())
            .unwrap_or(ctx.accounts.program.to_account_info()),
        // Optional minter_roles_pda: use actual account if exists, else use program ID
        ctx.accounts
            .minter_roles_pda
            .as_ref()
            .map(|acc| acc.to_account_info())
            .unwrap_or(ctx.accounts.program.to_account_info()),
        // Event CPI accounts
        ctx.accounts.event_authority.to_account_info(),
        ctx.accounts.program.to_account_info(),
    ];

    invoke_signed_with_its_root_pda(
        &deploy_instruction,
        &account_infos,
        ctx.accounts.its_root_pda.bump,
    )
}

fn invoke_signed_with_its_root_pda(
    instruction: &Instruction,
    account_infos: &[AccountInfo],
    its_root_pda_bump: u8,
) -> Result<()> {
    let seeds = &[InterchainTokenService::SEED_PREFIX, &[its_root_pda_bump]];
    let signer_seeds = &[&seeds[..]];

    anchor_lang::solana_program::program::invoke_signed(instruction, account_infos, signer_seeds)?;
    Ok(())
}
