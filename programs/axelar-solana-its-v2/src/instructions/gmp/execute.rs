use crate::{errors::ItsError, state::InterchainTokenService};
use anchor_lang::{prelude::*, InstructionData, Key};
use anchor_spl::{associated_token::AssociatedToken, token_interface::TokenInterface};
use axelar_solana_gateway_v2::{
    executable::{validate_message_raw, HasAxelarExecutable},
    executable_accounts, Message,
};
use interchain_token_transfer_gmp::GMPPayload;
use solana_program::instruction::Instruction;

executable_accounts!(Execute);

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
        constraint = !its_root_pda.paused @ ItsError::Paused
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

    #[account(address = anchor_lang::solana_program::sysvar::rent::ID)]
    pub rent: Sysvar<'info, Rent>,

    pub system_program: Program<'info, System>,

    // Remaining accounts
    #[account(mut)]
    pub deployer_ata: Option<UncheckedAccount<'info>>,
    #[account(mut)]
    pub deployer: Option<UncheckedAccount<'info>>,
    #[account(mut)]
    pub authority: Option<UncheckedAccount<'info>>,

    #[account(mut)]
    pub minter: Option<UncheckedAccount<'info>>,
    #[account(mut)]
    pub minter_roles_pda: Option<UncheckedAccount<'info>>,

    #[account(mut)]
    pub mpl_token_metadata_account: Option<UncheckedAccount<'info>>,
    pub mpl_token_metadata_program: Option<UncheckedAccount<'info>>,

    pub sysvar_instructions: Option<UncheckedAccount<'info>>,
    #[account(mut)]
    pub destination: Option<UncheckedAccount<'info>>,
    #[account(mut)]
    pub destination_ata: Option<UncheckedAccount<'info>>,
}

pub fn execute_handler(ctx: Context<Execute>, message: Message, payload: Vec<u8>) -> Result<()> {
    validate_message_raw(&ctx.accounts.axelar_executable(), message.clone(), &payload)?;

    msg!("execute_handler");
    // ITS specific logic

    if message.source_address != ctx.accounts.its_root_pda.its_hub_address {
        msg!("Untrusted source address: {}", message.source_address);
        return err!(ItsError::InvalidInstructionData);
    }

    let GMPPayload::ReceiveFromHub(inner_msg) =
        GMPPayload::decode(&payload).map_err(|_err| ProgramError::InvalidInstructionData)?
    else {
        msg!("Unsupported GMP payload");
        return err!(ItsError::InvalidInstructionData);
    };

    if !ctx
        .accounts
        .its_root_pda
        .is_trusted_chain(&inner_msg.source_chain)
    {
        return err!(ItsError::UntrustedSourceChain);
    }

    let payload = GMPPayload::decode(&inner_msg.payload)
        .map_err(|_err| ProgramError::InvalidInstructionData)?;

    perform_self_cpi(payload, ctx, message, &inner_msg.source_chain)?;

    Ok(())
}

fn perform_self_cpi(
    payload: GMPPayload,
    ctx: Context<Execute>,
    message: Message,
    source_chain: &str,
) -> Result<()> {
    match payload {
        GMPPayload::InterchainTransfer(transfer) => {
            interchain_transfer_self_invoke(ctx, transfer, message, source_chain)
        }
        GMPPayload::DeployInterchainToken(deploy) => {
            deploy_interchain_token_self_invoke(ctx, deploy)
        }
        GMPPayload::LinkToken(payload) => link_token_self_invoke(ctx, payload),
        GMPPayload::SendToHub(_)
        | GMPPayload::ReceiveFromHub(_)
        | GMPPayload::RegisterTokenMetadata(_) => err!(ItsError::InvalidInstructionData),
    }
}

fn interchain_transfer_self_invoke(
    ctx: Context<Execute>,
    transfer: interchain_token_transfer_gmp::InterchainTransfer,
    message: Message,
    source_chain: &str,
) -> Result<()> {
    let token_id = transfer.token_id.0;
    let source_address = String::from_utf8(transfer.source_address.to_vec())
        .map_err(|_| ProgramError::InvalidInstructionData)?;

    let destination_address: [u8; 32] = transfer
        .destination_address
        .as_ref()
        .try_into()
        .map_err(|_| ProgramError::InvalidAccountData)?;
    let destination_address = Pubkey::new_from_array(destination_address);

    let amount: u64 = transfer
        .amount
        .try_into()
        .map_err(|_| ProgramError::ArithmeticOverflow)?;

    let data = transfer.data;

    let instruction_data = crate::instruction::InterchainTransferInternal {
        token_id,
        source_address: source_address.clone(),
        destination_address,
        amount,
        data: data.to_vec(),
        message,
        source_chain: source_chain.to_owned(),
    };

    let transfer_instruction = Instruction {
        program_id: crate::id(),
        accounts: crate::accounts::InterchainTransferInternal {
            payer: ctx.accounts.payer.key(),
            authority: ctx.accounts.authority.clone().unwrap().key(),
            its_root_pda: ctx.accounts.its_root_pda.key(),
            destination: ctx.accounts.destination.clone().unwrap().key(),
            destination_ata: ctx.accounts.destination_ata.clone().unwrap().key(),
            token_mint: ctx.accounts.token_mint.key(),
            token_manager_pda: ctx.accounts.token_manager_pda.key(),
            token_manager_ata: ctx.accounts.token_manager_ata.key(),
            token_program: ctx.accounts.token_program.key(),
            // Event CPI accounts
            event_authority: ctx.accounts.event_authority.key(),
            program: ctx.accounts.program.key(),
        }
        .to_account_metas(None),
        data: instruction_data.data(),
    };

    let account_infos =
        crate::__cpi_client_accounts_interchain_transfer_internal::InterchainTransferInternal {
            payer: ctx.accounts.payer.to_account_info(),
            authority: ctx.accounts.authority.clone().unwrap().to_account_info(),
            its_root_pda: ctx.accounts.its_root_pda.to_account_info(),
            destination: ctx.accounts.destination.clone().unwrap().to_account_info(),
            destination_ata: ctx
                .accounts
                .destination_ata
                .clone()
                .unwrap()
                .to_account_info(),
            token_mint: ctx.accounts.token_mint.to_account_info(),
            token_manager_pda: ctx.accounts.token_manager_pda.to_account_info(),
            token_manager_ata: ctx.accounts.token_manager_ata.to_account_info(),
            token_program: ctx.accounts.token_program.to_account_info(),
            // Event CPI accounts
            event_authority: ctx.accounts.event_authority.to_account_info(),
            program: ctx.accounts.program.to_account_info(),
        }
        .to_account_infos();

    // Invoke the instruction with ITS root PDA as signer
    invoke_signed_with_its_root_pda(
        &transfer_instruction,
        &account_infos,
        ctx.accounts.its_root_pda.bump,
    )
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

    let accounts = crate::accounts::LinkTokenInternal {
        payer: ctx.accounts.payer.key(),
        deployer: ctx.accounts.deployer.clone().unwrap().key(),
        system_program: ctx.accounts.system_program.key(),
        its_root_pda: ctx.accounts.its_root_pda.key(),
        token_manager_pda: ctx.accounts.token_manager_pda.key(),
        token_mint: ctx.accounts.token_mint.key(),
        token_manager_ata: ctx.accounts.token_manager_ata.key(),
        token_program: ctx.accounts.token_program.key(),
        associated_token_program: ctx.accounts.associated_token_program.key(),
        rent: ctx.accounts.rent.key(),
        operator: ctx.accounts.minter.as_ref().map(Key::key), // Use minter as operator
        operator_roles_pda: ctx.accounts.minter_roles_pda.as_ref().map(Key::key),
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

    let account_infos = crate::__cpi_client_accounts_link_token_internal::LinkTokenInternal {
        payer: ctx.accounts.payer.to_account_info(),
        deployer: ctx.accounts.deployer.clone().unwrap().to_account_info(),
        system_program: ctx.accounts.system_program.to_account_info(),
        its_root_pda: ctx.accounts.its_root_pda.to_account_info(),
        token_manager_pda: ctx.accounts.token_manager_pda.to_account_info(),
        token_mint: ctx.accounts.token_mint.to_account_info(),
        token_manager_ata: ctx.accounts.token_manager_ata.to_account_info(),
        token_program: ctx.accounts.token_program.to_account_info(),
        associated_token_program: ctx.accounts.associated_token_program.to_account_info(),
        rent: ctx.accounts.rent.to_account_info(),
        operator: ctx
            .accounts
            .minter
            .as_ref()
            .map(ToAccountInfo::to_account_info),
        operator_roles_pda: ctx
            .accounts
            .minter_roles_pda
            .as_ref()
            .map(ToAccountInfo::to_account_info),
        // Event CPI accounts
        event_authority: ctx.accounts.event_authority.to_account_info(),
        program: ctx.accounts.program.to_account_info(),
    }
    .to_account_infos();

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
        deployer: ctx.accounts.deployer.clone().unwrap().key(),
        system_program: ctx.accounts.system_program.key(),
        its_root_pda: ctx.accounts.its_root_pda.key(),
        token_manager_pda: ctx.accounts.token_manager_pda.key(),
        token_mint: ctx.accounts.token_mint.key(),
        token_manager_ata: ctx.accounts.token_manager_ata.key(),
        token_program: ctx.accounts.token_program.key(),
        associated_token_program: ctx.accounts.associated_token_program.key(),
        rent: ctx.accounts.rent.key(),
        sysvar_instructions: ctx.accounts.sysvar_instructions.clone().unwrap().key(),
        mpl_token_metadata_program: ctx
            .accounts
            .mpl_token_metadata_program
            .clone()
            .unwrap()
            .key(),
        mpl_token_metadata_account: ctx
            .accounts
            .mpl_token_metadata_account
            .clone()
            .unwrap()
            .key(),
        deployer_ata: ctx.accounts.deployer_ata.clone().unwrap().key(),
        minter: ctx.accounts.minter.as_ref().map(Key::key),
        minter_roles_pda: ctx.accounts.minter_roles_pda.as_ref().map(Key::key),
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

    let account_infos = crate::__cpi_client_accounts_deploy_interchain_token_internal::DeployInterchainTokenInternal {
		payer: ctx.accounts.payer.to_account_info(),
		deployer: ctx.accounts.deployer.clone().unwrap().to_account_info(),
		system_program: ctx.accounts.system_program.to_account_info(),
		its_root_pda: ctx.accounts.its_root_pda.to_account_info(),
		token_manager_pda: ctx.accounts.token_manager_pda.to_account_info(),
		token_mint: ctx.accounts.token_mint.to_account_info(),
		token_manager_ata: ctx.accounts.token_manager_ata.to_account_info(),
		token_program: ctx.accounts.token_program.to_account_info(),
		associated_token_program: ctx.accounts.associated_token_program.to_account_info(),
		rent: ctx.accounts.rent.to_account_info(),
		sysvar_instructions: ctx.accounts.sysvar_instructions.clone().unwrap().to_account_info(),
		mpl_token_metadata_program: ctx
			.accounts
			.mpl_token_metadata_program
			.clone()
			.unwrap()
			.to_account_info(),
		mpl_token_metadata_account: ctx
			.accounts
			.mpl_token_metadata_account
			.clone()
			.unwrap()
			.to_account_info(),
		deployer_ata: ctx.accounts.deployer_ata.clone().unwrap().to_account_info(),
		minter: ctx
			.accounts
			.minter
			.as_ref()
			.map(ToAccountInfo::to_account_info),
		minter_roles_pda: ctx
			.accounts
			.minter_roles_pda
			.as_ref()
			.map(ToAccountInfo::to_account_info),
		// Event CPI accounts
		event_authority: ctx.accounts.event_authority.to_account_info(),
		program: ctx.accounts.program.to_account_info(),
	}.to_account_infos();

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
