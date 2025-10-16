use crate::{
    seed_prefixes::GOVERNANCE_CONFIG, GovernanceConfig, GovernanceError, OperatorshipTransferred,
};
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[event_cpi]
pub struct TransferOperatorship<'info> {
    pub system_program: Program<'info, System>,

    /// The current operator account - may or may not be a signer
    /// CHECK: We manually validate this account based on signing requirements
    pub operator_account: Option<Signer<'info>>,

    #[account(
        mut,
        seeds = [GOVERNANCE_CONFIG],
        bump = governance_config.bump,
        // Either the operator_account is a signer and matches the stored operator,
		// or the governance_config account itself is a signer (program root)
        constraint = operator_account.as_ref().is_some_and(|op| op.key.to_bytes() == governance_config.operator)
            || governance_config.to_account_info().is_signer
            @ GovernanceError::MissingRequiredSignature,
    )]
    pub governance_config: Account<'info, GovernanceConfig>,
}

pub fn transfer_operatorship_handler(
    ctx: Context<TransferOperatorship>,
    new_operator: [u8; 32],
) -> Result<()> {
    let config = &mut ctx.accounts.governance_config;

    let old_operator = config.operator;
    config.operator = new_operator;

    emit_cpi!(OperatorshipTransferred {
        old_operator: old_operator.to_vec(),
        new_operator: new_operator.to_vec(),
    });

    Ok(())
}
