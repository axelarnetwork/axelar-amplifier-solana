use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct EmitMemo<'info> {
    /// CHECK: will be removed
    pub some_account: UncheckedAccount<'info>,
}

pub fn emit_memo_handler(_ctx: Context<EmitMemo>, message: String) -> Result<()> {
    msg!("Received memo: {}", message);
    Ok(())
}
