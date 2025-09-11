use crate::{GatewayConfig, GatewayError, OperatorshipTransferedEvent};
use anchor_lang::prelude::*;
use axelar_solana_gateway::seed_prefixes::GATEWAY_SEED;
use solana_program::bpf_loader_upgradeable;

#[derive(Accounts)]
#[event_cpi]
pub struct TransferOperatorship<'info> {
    #[account(
            mut,
            seeds = [GATEWAY_SEED],
            bump = gateway_root_pda.bump
        )]
    pub gateway_root_pda: Account<'info, GatewayConfig>,
    pub operator_or_upgrade_authority: Signer<'info>,
    #[account(
            constraint = programdata_account.key() ==
                Pubkey::find_program_address(&[crate::ID.as_ref()], &bpf_loader_upgradeable::id()).0
                @ GatewayError::InvalidUpgradeAuthority
        )]
    pub programdata_account: UncheckedAccount<'info>,
    pub new_operator: UncheckedAccount<'info>,
}

pub fn transfer_operatorship_handler(ctx: Context<TransferOperatorship>) -> Result<()> {
    // Check: the programda state is valid
    let loader_state = ctx
        .accounts
        .programdata_account
        .data
        .borrow()
        .get(0..UpgradeableLoaderState::size_of_programdata_metadata())
        .ok_or(GatewayError::InvalidLoaderContent)
        .and_then(|bytes: &[u8]| {
            bincode::deserialize::<UpgradeableLoaderState>(bytes)
                .map_err(|_err| GatewayError::InvalidLoaderContent)
        })?;

    let UpgradeableLoaderState::ProgramData {
        upgrade_authority_address,
        ..
    } = loader_state
    else {
        return err!(GatewayError::InvalidLoaderState);
    };

    // Check: the signer matches either the current operator or the upgrade authority
    if !(ctx.accounts.gateway_root_pda.operator == *ctx.accounts.operator_or_upgrade_authority.key
        || upgrade_authority_address == Some(*ctx.accounts.operator_or_upgrade_authority.key))
    {
        return err!(GatewayError::InvalidOperatorOrAuthorityAccount);
    }

    ctx.accounts.gateway_root_pda.operator = *ctx.accounts.new_operator.key;

    emit_cpi!(OperatorshipTransferedEvent {
        new_operator: ctx.accounts.gateway_root_pda.operator.to_bytes()
    });

    Ok(())
}
