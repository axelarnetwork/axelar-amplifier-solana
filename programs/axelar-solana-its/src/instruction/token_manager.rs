//! Instructions for the token manager.

use borsh::to_vec;
use solana_program::hash;
use solana_program::instruction::AccountMeta;
use solana_program::program_error::ProgramError;
use solana_program::pubkey::Pubkey;
use solana_sdk_ids::system_program;

use super::InterchainTokenServiceInstruction;

/// Creates an [`TokenManagerInstructions::SetFlowLimit`] wrapped in an
/// [`InterchainTokenServiceInstruction::TokenManagerInstruction`].
///
/// # Errors
///
/// If serialization fails.
pub fn set_flow_limit(
    payer: Pubkey,
    token_id: [u8; 32],
    flow_limit: u64,
) -> Result<solana_program::instruction::Instruction, ProgramError> {
    let (its_root_pda, _) = crate::find_its_root_pda();
    let (token_manager_pda, _) = crate::find_token_manager_pda(&its_root_pda, &token_id);
    let (token_manager_user_roles_pda, _) =
        role_management::find_user_roles_pda(&crate::id(), &token_manager_pda, &payer);
    let (its_user_roles_pda, _) =
        role_management::find_user_roles_pda(&crate::id(), &its_root_pda, &payer);

    let instruction_data =
        to_vec(&InterchainTokenServiceInstruction::SetTokenManagerFlowLimit { flow_limit })?;

    let discriminator: [u8; 8] = hash::hash(b"global:set_token_manager_flow_limit").to_bytes()[..8]
        .try_into()
        .unwrap();

    let data: Vec<u8> = discriminator
        .iter()
        .chain(instruction_data.iter())
        .cloned()
        .collect();

    let accounts = vec![
        AccountMeta::new_readonly(payer, true),
        AccountMeta::new_readonly(its_root_pda, false),
        AccountMeta::new(token_manager_pda, false),
        AccountMeta::new_readonly(token_manager_user_roles_pda, false),
        AccountMeta::new_readonly(its_user_roles_pda, false),
        AccountMeta::new_readonly(system_program::ID, false),
    ];

    Ok(solana_program::instruction::Instruction {
        program_id: crate::id(),
        accounts,
        data,
    })
}

/// Creates a [`TokenManagerInstructions::AddFlowLimiter`] instruction.
///
/// # Errors
///
/// If serialization fails.
pub fn add_flow_limiter(
    payer: Pubkey,
    token_id: [u8; 32],
    flow_limiter: Pubkey,
) -> Result<solana_program::instruction::Instruction, ProgramError> {
    let (its_root_pda, _) = crate::find_its_root_pda();
    let (token_manager_pda, _) = crate::find_token_manager_pda(&its_root_pda, &token_id);
    let (payer_roles_pda, _) =
        role_management::find_user_roles_pda(&crate::id(), &token_manager_pda, &payer);
    let (flow_limiter_roles_pda, _) =
        role_management::find_user_roles_pda(&crate::id(), &token_manager_pda, &flow_limiter);

    let accounts = vec![
        AccountMeta::new_readonly(its_root_pda, false),
        AccountMeta::new_readonly(system_program::ID, false),
        AccountMeta::new(payer, true),
        AccountMeta::new_readonly(payer_roles_pda, false),
        AccountMeta::new_readonly(token_manager_pda, false),
        AccountMeta::new_readonly(flow_limiter, false),
        AccountMeta::new(flow_limiter_roles_pda, false),
    ];

    let instruction_data = to_vec(&InterchainTokenServiceInstruction::AddTokenManagerFlowLimiter)?;

    let discriminator: [u8; 8] = hash::hash(b"global:add_token_manager_flow_limiter").to_bytes()
        [..8]
        .try_into()
        .unwrap();

    let data: Vec<u8> = discriminator
        .iter()
        .chain(instruction_data.iter())
        .cloned()
        .collect();

    Ok(solana_program::instruction::Instruction {
        program_id: crate::id(),
        accounts,
        data,
    })
}

/// Creates a [`TokenManagerInstructions::RemoveFlowLimiter`] instruction.
///
/// # Errors
///
/// If serialization fails.
pub fn remove_flow_limiter(
    payer: Pubkey,
    token_id: [u8; 32],
    flow_limiter: Pubkey,
) -> Result<solana_program::instruction::Instruction, ProgramError> {
    let (its_root_pda, _) = crate::find_its_root_pda();
    let (token_manager_pda, _) = crate::find_token_manager_pda(&its_root_pda, &token_id);
    let (payer_roles_pda, _) =
        role_management::find_user_roles_pda(&crate::id(), &token_manager_pda, &payer);
    let (flow_limiter_roles_pda, _) =
        role_management::find_user_roles_pda(&crate::id(), &token_manager_pda, &flow_limiter);

    let accounts = vec![
        AccountMeta::new_readonly(its_root_pda, false),
        AccountMeta::new_readonly(system_program::ID, false),
        AccountMeta::new(payer, true),
        AccountMeta::new_readonly(payer_roles_pda, false),
        AccountMeta::new_readonly(token_manager_pda, false),
        AccountMeta::new_readonly(flow_limiter, false),
        AccountMeta::new(flow_limiter_roles_pda, false),
    ];

    let instruction_data =
        to_vec(&InterchainTokenServiceInstruction::RemoveTokenManagerFlowLimiter)?;

    let discriminator: [u8; 8] = hash::hash(b"global:remove_token_manager_flow_limiter").to_bytes()
        [..8]
        .try_into()
        .unwrap();

    let data: Vec<u8> = discriminator
        .iter()
        .chain(instruction_data.iter())
        .cloned()
        .collect();

    Ok(solana_program::instruction::Instruction {
        program_id: crate::id(),
        accounts,
        data,
    })
}

/// Creates an [`InterchainTokenServiceInstruction::TransferTokenManagerOperatorship`] instruction.
///
/// # Errors
///
/// If serialization fails.
pub fn transfer_operatorship(
    payer: Pubkey,
    token_id: [u8; 32],
    to: Pubkey,
) -> Result<solana_program::instruction::Instruction, ProgramError> {
    let (its_root_pda, _) = crate::find_its_root_pda();
    let (token_manager_pda, _) = crate::find_token_manager_pda(&its_root_pda, &token_id);
    let (destination_roles_pda, _) =
        role_management::find_user_roles_pda(&crate::id(), &token_manager_pda, &to);
    let (payer_roles_pda, _) =
        role_management::find_user_roles_pda(&crate::id(), &token_manager_pda, &payer);

    let accounts = vec![
        AccountMeta::new_readonly(its_root_pda, false),
        AccountMeta::new_readonly(solana_program::system_program::id(), false),
        AccountMeta::new(payer, true),
        AccountMeta::new(payer_roles_pda, false),
        AccountMeta::new_readonly(token_manager_pda, false),
        AccountMeta::new_readonly(to, false),
        AccountMeta::new(destination_roles_pda, false),
    ];

    let instruction_data =
        to_vec(&InterchainTokenServiceInstruction::TransferTokenManagerOperatorship)?;

    let discriminator: [u8; 8] = hash::hash(b"global:transfer_token_manager_operatorship")
        .to_bytes()[..8]
        .try_into()
        .unwrap();

    let data: Vec<u8> = discriminator
        .iter()
        .chain(instruction_data.iter())
        .cloned()
        .collect();

    Ok(solana_program::instruction::Instruction {
        program_id: crate::id(),
        accounts,
        data,
    })
}

/// Creates an [`InterchainTokenServiceInstruction::ProposeTokenManagerOperatorship`] instruction.
///
/// # Errors
///
/// If serialization fails.
pub fn propose_operatorship(
    payer: Pubkey,
    token_id: [u8; 32],
    to: Pubkey,
) -> Result<solana_program::instruction::Instruction, ProgramError> {
    let (its_root_pda, _) = crate::find_its_root_pda();
    let (token_manager_pda, _) = crate::find_token_manager_pda(&its_root_pda, &token_id);
    let (payer_roles_pda, _) =
        role_management::find_user_roles_pda(&crate::id(), &token_manager_pda, &payer);
    let (destination_roles_pda, _) =
        role_management::find_user_roles_pda(&crate::id(), &token_manager_pda, &to);
    let (proposal_pda, _) =
        role_management::find_roles_proposal_pda(&crate::id(), &token_manager_pda, &payer, &to);

    let accounts = vec![
        AccountMeta::new_readonly(its_root_pda, false),
        AccountMeta::new_readonly(solana_program::system_program::id(), false),
        AccountMeta::new(payer, true),
        AccountMeta::new_readonly(payer_roles_pda, false),
        AccountMeta::new_readonly(token_manager_pda, false),
        AccountMeta::new_readonly(to, false),
        AccountMeta::new(destination_roles_pda, false),
        AccountMeta::new(proposal_pda, false),
    ];

    let instruction_data =
        to_vec(&InterchainTokenServiceInstruction::ProposeTokenManagerOperatorship)?;

    let discriminator: [u8; 8] = hash::hash(b"global:propose_token_manager_operatorship")
        .to_bytes()[..8]
        .try_into()
        .unwrap();

    let data: Vec<u8> = discriminator
        .iter()
        .chain(instruction_data.iter())
        .cloned()
        .collect();

    Ok(solana_program::instruction::Instruction {
        program_id: crate::id(),
        accounts,
        data,
    })
}

/// Creates an [`InterchainTokenServiceInstruction::AcceptTokenManagerOperatorship`] instruction.
///
/// # Errors
///
/// If serialization fails.
pub fn accept_operatorship(
    payer: Pubkey,
    token_id: [u8; 32],
    from: Pubkey,
) -> Result<solana_program::instruction::Instruction, ProgramError> {
    let (its_root_pda, _) = crate::find_its_root_pda();
    let (token_manager_pda, _) = crate::find_token_manager_pda(&its_root_pda, &token_id);
    let (payer_roles_pda, _) =
        role_management::find_user_roles_pda(&crate::id(), &token_manager_pda, &payer);
    let (origin_roles_pda, _) =
        role_management::find_user_roles_pda(&crate::id(), &token_manager_pda, &from);
    let (proposal_pda, _) =
        role_management::find_roles_proposal_pda(&crate::id(), &token_manager_pda, &from, &payer);

    let accounts = vec![
        AccountMeta::new_readonly(its_root_pda, false),
        AccountMeta::new_readonly(solana_program::system_program::id(), false),
        AccountMeta::new(payer, true),
        AccountMeta::new(payer_roles_pda, false),
        AccountMeta::new_readonly(token_manager_pda, false),
        AccountMeta::new_readonly(from, false),
        AccountMeta::new(origin_roles_pda, false),
        AccountMeta::new(proposal_pda, false),
    ];

    let instruction_data =
        to_vec(&InterchainTokenServiceInstruction::AcceptTokenManagerOperatorship)?;

    let discriminator: [u8; 8] = hash::hash(b"global:accept_token_manager_operatorship").to_bytes()
        [..8]
        .try_into()
        .unwrap();

    let data: Vec<u8> = discriminator
        .iter()
        .chain(instruction_data.iter())
        .cloned()
        .collect();

    Ok(solana_program::instruction::Instruction {
        program_id: crate::id(),
        accounts,
        data,
    })
}

/// Creates an [`InterchainTokenServiceInstruction::HandoverMintAuthority`] instruction.
///
/// # Errors
///
/// If serialization fails.
pub fn handover_mint_authority(
    payer: Pubkey,
    token_id: [u8; 32],
    mint: Pubkey,
    token_program: Pubkey,
) -> Result<solana_program::instruction::Instruction, ProgramError> {
    let (its_root_pda, _) = crate::find_its_root_pda();
    let (token_manager_pda, _) = crate::find_token_manager_pda(&its_root_pda, &token_id);
    let (minter_roles_pda, _) =
        role_management::find_user_roles_pda(&crate::ID, &token_manager_pda, &payer);

    let accounts = vec![
        AccountMeta::new(payer, true),
        AccountMeta::new(mint, false),
        AccountMeta::new_readonly(its_root_pda, false),
        AccountMeta::new_readonly(token_manager_pda, false),
        AccountMeta::new(minter_roles_pda, false),
        AccountMeta::new_readonly(token_program, false),
        AccountMeta::new_readonly(system_program::ID, false),
    ];

    let instruction_data =
        to_vec(&InterchainTokenServiceInstruction::HandoverMintAuthority { token_id })?;

    let discriminator: [u8; 8] = hash::hash(b"global:handover_mint_authority").to_bytes()[..8]
        .try_into()
        .unwrap();

    let data: Vec<u8> = discriminator
        .iter()
        .chain(instruction_data.iter())
        .cloned()
        .collect();

    Ok(solana_program::instruction::Instruction {
        program_id: crate::id(),
        accounts,
        data,
    })
}
