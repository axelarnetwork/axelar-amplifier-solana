use crate::state::*;
use crate::ErrorCode;
use crate::MasterTransferred;
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct TransferMaster<'info> {
    #[account(
        mut,
        address = registry.master_operator @ ErrorCode::UnauthorizedMaster
    )]
    pub current_master: Signer<'info>,

    // TODO(v2) either change this to Signer
    // or introduce a propose/accept flow
    //
    /// CHECK: The new master operator pubkey
    #[account(
    	// Ensure the new master is not the same as the current master
		constraint = new_master.key() != registry.master_operator @ ErrorCode::SameMaster
    )]
    pub new_master: AccountInfo<'info>,

    #[account(
        mut,
        seeds = [OperatorRegistry::SEED_PREFIX],
        bump = registry.bump,
    )]
    pub registry: Account<'info, OperatorRegistry>,
}

pub fn transfer_master(ctx: Context<TransferMaster>) -> Result<()> {
    let registry = &mut ctx.accounts.registry;

    // Update the master operator
    registry.master_operator = ctx.accounts.new_master.key();

    emit!(MasterTransferred {
        old_master: ctx.accounts.current_master.key(),
        new_master: ctx.accounts.new_master.key(),
    });

    Ok(())
}
