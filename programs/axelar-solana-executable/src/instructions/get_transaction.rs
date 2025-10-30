use anchor_lang::prelude::*;
use relayer_discovery::{structs::{RelayerData, RelayerInstruction, RelayerTransaction}};

#[derive(Accounts)]
pub struct GetTransaction<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        mut,
        seeds = [
            &relayer_discovery::TRANSACTION_PDA_SEED,
        ],
        bump = relayer_discovery::find_transaction_pda(&crate::id()).1,
        // CHECK: Validate signature verification session is complete
    )]
    pub relayer_transaction: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
}

pub fn get_transaction_handler(ctx: Context<GetTransaction>) -> Result<RelayerTransaction> {
    relayer_transaction().init(
        &crate::id(),
        &ctx.accounts.system_program.to_account_info(),
        &ctx.accounts.payer.to_account_info(),
        &ctx.accounts.relayer_transaction,
    )?;
    Ok(RelayerTransaction::Final(vec![
      RelayerInstruction {
        program_id: crate::id(),
        accounts: vec![

        ],
        data: vec! [
            
        ]
      }  
    ]))
}

fn relayer_transaction() -> RelayerTransaction {
    RelayerTransaction::Discovery(RelayerInstruction {
        program_id: crate::ID,
        accounts: vec![],
        data: vec![
            RelayerData::Bytes(vec![]),
            RelayerData::Payload,
        ],
    })
}
