use crate::{GovernanceConfig, GovernanceError, OperatorshipTransferred};
use anchor_lang::prelude::*;
use axelar_solana_governance::seed_prefixes;

#[derive(Accounts)]
#[event_cpi]
pub struct TransferOperatorship<'info> {
    pub system_program: Program<'info, System>,
    /// The current operator account - may or may not be a signer
    /// CHECK: We manually validate this account based on signing requirements
    pub operator_account: AccountInfo<'info>,
    #[account(
        mut,
        seeds = [seed_prefixes::GOVERNANCE_CONFIG],
        bump = governance_config.bump,
    )]
    pub governance_config: Account<'info, GovernanceConfig>,
}

pub fn transfer_operatorship_handler(
    ctx: Context<TransferOperatorship>,
    new_operator: [u8; 32],
) -> Result<()> {
    let config = &mut ctx.accounts.governance_config;
    let operator_account = &ctx.accounts.operator_account;
    let config_pda = &config.to_account_info();

    if !(operator_account.is_signer || config_pda.is_signer) {
        msg!("The operator account or program root account, must sign the transaction");
        return err!(GovernanceError::MissingRequiredSignature);
    }

    if operator_account.is_signer && operator_account.key.to_bytes() != config.operator {
        msg!("Operator account must sign the transaction");
        return err!(GovernanceError::MissingRequiredSignature);
    }

    let old_operator = config.operator;
    config.operator = new_operator;

    emit_cpi!(OperatorshipTransferred {
        old_operator: old_operator.to_vec(),
        new_operator: new_operator.to_vec(),
    });

    Ok(())
}
