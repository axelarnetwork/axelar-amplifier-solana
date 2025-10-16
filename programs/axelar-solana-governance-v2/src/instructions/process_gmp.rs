use crate::program::AxelarSolanaGovernanceV2;
use crate::{ExecutableProposal, ExecuteProposalCallData, GovernanceConfig, GovernanceError};
use anchor_lang::{prelude::*, solana_program, InstructionData};
use axelar_solana_gateway_v2::seed_prefixes::{
    INCOMING_MESSAGE_SEED, VALIDATE_MESSAGE_SIGNING_SEED,
};
use axelar_solana_gateway_v2::{
    cpi::accounts::ValidateMessage, program::AxelarSolanaGatewayV2, IncomingMessage, Message,
};
use governance_gmp::{GovernanceCommand, GovernanceCommandPayload};
use solana_program::instruction::Instruction;

#[derive(Accounts)]
#[instruction(message: Message, payload: Vec<u8>)]
pub struct ProcessGmp<'info> {
    #[account(
        seeds = [INCOMING_MESSAGE_SEED, message.command_id().as_ref()],
        bump = incoming_message_pda.load()?.bump,
        seeds::program = axelar_gateway_program.key()
    )]
    pub incoming_message_pda: AccountLoader<'info, IncomingMessage>,

    /// Signing PDA for this program - used to validate with gateway
    #[account(
        mut,
        signer,
        seeds = [VALIDATE_MESSAGE_SIGNING_SEED, message.command_id().as_ref()],
        bump = incoming_message_pda.load()?.signing_pda_bump,
    )]
    pub signing_pda: AccountInfo<'info>,

    pub axelar_gateway_program: Program<'info, AxelarSolanaGatewayV2>,

    #[account(
        seeds = [b"__event_authority"],
        bump,
        seeds::program = crate::ID.key()
    )]
    pub governance_event_authority: SystemAccount<'info>,

    pub axelar_governance_program: Program<'info, AxelarSolanaGovernanceV2>,

    #[account(
        seeds = [b"__event_authority"],
        bump,
        seeds::program = axelar_gateway_program.key()
    )]
    pub gateway_event_authority: SystemAccount<'info>,

    pub system_program: Program<'info, System>,

    #[account(
    	seeds = [GovernanceConfig::SEED_PREFIX],
        bump = governance_config.load()?.bump
    )]
    pub governance_config: AccountLoader<'info, GovernanceConfig>,

    #[account(mut)]
    pub payer: Signer<'info>,

    // Variable accounts as kept as unchecked. We self-CPI and check them for each separate instruction
    #[account(mut)]
    pub proposal_pda: UncheckedAccount<'info>,

    #[account(mut)]
    pub operator_proposal_pda: UncheckedAccount<'info>,
}

pub fn process_gmp_handler(
    ctx: Context<ProcessGmp>,
    message: Message,
    payload: Vec<u8>,
) -> Result<()> {
    // Check that provided payload matches the approved message
    let computed_payload_hash = solana_program::keccak::hashv(&[&payload]).to_bytes();
    if computed_payload_hash != message.payload_hash {
        return err!(GovernanceError::InvalidPayloadHash);
    }

    let cpi_accounts = ValidateMessage {
        incoming_message_pda: ctx.accounts.incoming_message_pda.to_account_info(),
        caller: ctx.accounts.signing_pda.to_account_info(),
        // for emit cpi
        event_authority: ctx.accounts.gateway_event_authority.to_account_info(),
        program: ctx.accounts.axelar_gateway_program.to_account_info(),
    };

    let binding = message.command_id();
    let msg = binding.as_ref();

    let seeds = &[
        VALIDATE_MESSAGE_SIGNING_SEED,
        msg,
        &[ctx.accounts.incoming_message_pda.load()?.signing_pda_bump],
    ];
    let signer_seeds = &[&seeds[..]];

    let cpi_ctx = CpiContext::new_with_signer(
        ctx.accounts.axelar_gateway_program.to_account_info(),
        cpi_accounts,
        signer_seeds,
    );

    axelar_solana_gateway_v2::cpi::validate_message(cpi_ctx, message.clone())?;

    {
        let config = ctx.accounts.governance_config.load()?;
        ensure_authorized_gmp_command(&config, &message)?;
    }

    let (cmd_payload, _, _, proposal_hash) = calculate_gmp_context(payload)?;

    process_proposal_gmp(ctx, proposal_hash, cmd_payload)
}

fn ensure_authorized_gmp_command(config: &GovernanceConfig, message: &Message) -> Result<()> {
    // Ensure the incoming address matches stored configuration.
    let address_hash =
        solana_program::keccak::hashv(&[message.source_address.as_bytes()]).to_bytes();
    if address_hash != config.address_hash {
        msg!(
            "Incoming governance GMP message came with non authorized address: {}",
            message.source_address
        );
        return err!(GovernanceError::UnauthorizedAddress);
    }

    // Ensure the incoming chain matches stored configuration.
    let chain_hash = solana_program::keccak::hashv(&[message.cc_id.chain.as_bytes()]).to_bytes();
    if chain_hash != config.chain_hash {
        msg!(
            "Incoming governance GMP message came with non authorized chain: {}",
            message.cc_id.chain
        );
        return err!(GovernanceError::UnauthorizedChain);
    }

    Ok(())
}

fn calculate_gmp_context(
    payload: Vec<u8>,
) -> Result<(
    GovernanceCommandPayload,
    Pubkey,
    ExecuteProposalCallData,
    [u8; 32],
)> {
    let cmd_payload = crate::decode_payload(&payload).unwrap();

    let target = crate::decode_payload_target(&cmd_payload.target)?;

    let execute_proposal_call_data = crate::decode_payload_call_data(&cmd_payload.call_data)?;

    let proposal_hash = ExecutableProposal::calculate_hash(
        &target,
        &execute_proposal_call_data,
        &cmd_payload.native_value.to_le_bytes(),
    );

    Ok((
        cmd_payload,
        target,
        execute_proposal_call_data,
        proposal_hash,
    ))
}

fn process_proposal_gmp(
    ctx: Context<ProcessGmp>,
    proposal_hash: [u8; 32],
    cmd_payload: GovernanceCommandPayload,
) -> Result<()> {
    match cmd_payload.command {
        GovernanceCommand::ScheduleTimeLockProposal => {
            schedule_timelock_proposal(ctx, cmd_payload, proposal_hash)
        }
        GovernanceCommand::CancelTimeLockProposal => {
            cancel_timelock_proposal(ctx, cmd_payload, proposal_hash)
        }
        GovernanceCommand::ApproveOperatorProposal => {
            approve_operator_proposal(ctx, cmd_payload, proposal_hash)
        }
        GovernanceCommand::CancelOperatorApproval => {
            cancel_operator_proposal(ctx, cmd_payload, proposal_hash)
        }
        _ => {
            msg!("Governance GMP command cannot be processed");
            err!(GovernanceError::InvalidInstructionData)
        }
    }
}

#[allow(clippy::type_complexity)]
fn extract_proposal_data(
    cmd_payload: GovernanceCommandPayload,
) -> Result<(u64, Vec<u8>, Vec<u8>, Vec<u8>)> {
    let eta = cmd_payload
        .eta
        .try_into()
        .map_err(|_| GovernanceError::InvalidInstructionData)?;
    let native_value = cmd_payload.native_value.to_le_bytes::<32>().to_vec();
    let call_data = cmd_payload.call_data.into();
    let target = cmd_payload.target.to_vec();

    Ok((eta, native_value, call_data, target))
}

fn schedule_timelock_proposal(
    ctx: Context<ProcessGmp>,
    cmd_payload: GovernanceCommandPayload,
    proposal_hash: [u8; 32],
) -> Result<()> {
    let (eta, native_value, call_data, target) = extract_proposal_data(cmd_payload)?;

    let instruction_data = crate::instruction::ScheduleTimelockProposal {
        proposal_hash,
        eta,
        target,
        native_value,
        call_data,
    };

    let schedule_instruction = Instruction {
        program_id: crate::ID,
        accounts: crate::accounts::ScheduleTimelockProposal {
            system_program: ctx.accounts.system_program.key(),
            governance_config: ctx.accounts.governance_config.key(),
            payer: ctx.accounts.payer.key(),
            proposal_pda: ctx.accounts.proposal_pda.key(),
            // for event cpi
            event_authority: ctx.accounts.governance_event_authority.key(),
            program: ctx.accounts.axelar_governance_program.key(),
        }
        .to_account_metas(None),
        data: instruction_data.data(),
    };

    let account_infos = vec![
        ctx.accounts.system_program.to_account_info(),
        ctx.accounts.governance_config.to_account_info(),
        ctx.accounts.payer.to_account_info(),
        ctx.accounts.proposal_pda.to_account_info(),
        // for emit cpi
        ctx.accounts.governance_event_authority.to_account_info(),
        ctx.accounts.axelar_governance_program.to_account_info(),
    ];

    invoke_signed_with_governance_config(
        &schedule_instruction,
        &account_infos,
        ctx.accounts.governance_config.load()?.bump,
    )
}

fn cancel_timelock_proposal(
    ctx: Context<ProcessGmp>,
    cmd_payload: GovernanceCommandPayload,
    proposal_hash: [u8; 32],
) -> Result<()> {
    let (eta, native_value, call_data, target) = extract_proposal_data(cmd_payload)?;

    let instruction_data = crate::instruction::CancelTimelockProposal {
        proposal_hash,
        eta,
        native_value,
        call_data,
        target,
    };

    // Create the instruction with accounts matching CancelTimelockProposal
    let cancel_instruction = Instruction {
        program_id: crate::ID,
        accounts: crate::accounts::CancelTimelockProposal {
            governance_config: ctx.accounts.governance_config.key(),
            proposal_pda: ctx.accounts.proposal_pda.key(),
            // for event cpi
            event_authority: ctx.accounts.governance_event_authority.key(),
            program: ctx.accounts.axelar_governance_program.key(),
        }
        .to_account_metas(None),
        data: instruction_data.data(),
    };

    // Account infos for the CPI call
    let account_infos = vec![
        ctx.accounts.governance_config.to_account_info(),
        ctx.accounts.proposal_pda.to_account_info(),
        // for emit cpi
        ctx.accounts.governance_event_authority.to_account_info(),
        ctx.accounts.axelar_governance_program.to_account_info(),
    ];

    invoke_signed_with_governance_config(
        &cancel_instruction,
        &account_infos,
        ctx.accounts.governance_config.load()?.bump,
    )
}

fn approve_operator_proposal(
    ctx: Context<ProcessGmp>,
    cmd_payload: GovernanceCommandPayload,
    proposal_hash: [u8; 32],
) -> Result<()> {
    let (_, native_value, call_data, target) = extract_proposal_data(cmd_payload)?;

    let instruction_data = crate::instruction::ApproveOperatorProposal {
        proposal_hash,
        native_value,
        call_data,
        target,
    };

    // Create the instruction with accounts matching ApproveOperatorProposal
    let approve_instruction = Instruction {
        program_id: crate::ID,
        accounts: crate::accounts::ApproveOperatorProposal {
            system_program: ctx.accounts.system_program.key(),
            governance_config: ctx.accounts.governance_config.key(),
            payer: ctx.accounts.payer.key(),
            proposal_pda: ctx.accounts.proposal_pda.key(),
            operator_proposal_pda: ctx.accounts.operator_proposal_pda.key(),
            // for event cpi
            event_authority: ctx.accounts.governance_event_authority.key(),
            program: ctx.accounts.axelar_governance_program.key(),
        }
        .to_account_metas(None),
        data: instruction_data.data(),
    };

    // Account infos for the CPI call
    let account_infos = vec![
        ctx.accounts.system_program.to_account_info(),
        ctx.accounts.governance_config.to_account_info(),
        ctx.accounts.payer.to_account_info(),
        ctx.accounts.proposal_pda.to_account_info(),
        ctx.accounts.operator_proposal_pda.to_account_info(),
        // for emit cpi
        ctx.accounts.governance_event_authority.to_account_info(),
        ctx.accounts.axelar_governance_program.to_account_info(),
    ];

    invoke_signed_with_governance_config(
        &approve_instruction,
        &account_infos,
        ctx.accounts.governance_config.load()?.bump,
    )
}

fn cancel_operator_proposal(
    ctx: Context<ProcessGmp>,
    cmd_payload: GovernanceCommandPayload,
    proposal_hash: [u8; 32],
) -> Result<()> {
    let (_, native_value, call_data, target) = extract_proposal_data(cmd_payload)?;

    let instruction_data = crate::instruction::CancelOperatorProposal {
        proposal_hash,
        native_value,
        call_data,
        target,
    };

    // Create the instruction with accounts matching CancelOperatorProposal
    let cancel_instruction = Instruction {
        program_id: crate::ID,
        accounts: crate::accounts::CancelOperatorProposal {
            governance_config: ctx.accounts.governance_config.key(),
            proposal_pda: ctx.accounts.proposal_pda.key(),
            operator_proposal_pda: ctx.accounts.operator_proposal_pda.key(),
            // for event cpi
            event_authority: ctx.accounts.governance_event_authority.key(),
            program: ctx.accounts.axelar_governance_program.key(),
        }
        .to_account_metas(None),
        data: instruction_data.data(),
    };

    // Account infos for the CPI call
    let account_infos = vec![
        ctx.accounts.governance_config.to_account_info(),
        ctx.accounts.proposal_pda.to_account_info(),
        ctx.accounts.operator_proposal_pda.to_account_info(),
        // for emit cpi
        ctx.accounts.governance_event_authority.to_account_info(),
        ctx.accounts.axelar_governance_program.to_account_info(),
    ];

    invoke_signed_with_governance_config(
        &cancel_instruction,
        &account_infos,
        ctx.accounts.governance_config.load()?.bump,
    )
}

fn invoke_signed_with_governance_config(
    instruction: &Instruction,
    account_infos: &[AccountInfo],
    governance_config_bump: u8,
) -> Result<()> {
    let seeds = &[GovernanceConfig::SEED_PREFIX, &[governance_config_bump]];
    let signer_seeds = &[&seeds[..]];

    solana_program::program::invoke_signed(instruction, account_infos, signer_seeds)?;
    Ok(())
}
