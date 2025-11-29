/// Macro to generate executable accounts and validation function.
/// Usage:
/// ```ignore
/// use anchor_lang::prelude::*;
/// use solana_axelar_gateway::{executable::*, executable_accounts};
///
/// executable_accounts!(Execute);
///
/// #[derive(Accounts)]
/// pub struct Execute<'info> {
///     // GMP Accounts
///     pub executable: AxelarExecuteAccounts<'info>,
///
///     // Your program accounts here
/// }
///
/// pub fn execute_handler(ctx: Context<Execute>, message: Message, payload: Vec<u8>) -> Result<()> {
///     validate_message(&ctx.accounts, message, &payload)?;
///
///     Ok(())
/// }
/// ```
///
/// NOTE: Keep in mind the outer accounts struct must not include:
/// ```ignore
/// #[instruction(message: Message, payload: Vec<u8>)]
/// ```
/// attribute due to [a bug](https://github.com/solana-foundation/anchor/issues/2942) in Anchor.
// NOTE: This macro is necessary because Anchor currently does not support importing
// accounts from other crates. Once Anchor supports this, we can remove this macro and
// export the accounts directly from solana-axelar-gateway.
// See: https://github.com/solana-foundation/anchor/issues/3811
// It is also not possible to use the `cpi` module inside the gateway crate.
#[macro_export]
macro_rules! transaction_pda_accounts {
    ($transaction:expr, $seed:expr) => {
        /// Accounts for register the initial relayer transaction.
        #[derive(Accounts)]
        pub struct RelayerTransactionAccounts<'info> {
            #[account(mut)]
            pub payer: Signer<'info>,

            // IncomingMessage PDA account
            // needs to be mutable as the validate_message CPI
            // updates its state
            #[account(
                                init,
                                seeds = [$seed],
                                bump,
                                payer = payer,
                                space = {
                                    let mut bytes = Vec::with_capacity(256);
                                    $transaction.serialize(&mut bytes)?;
                                    bytes.len()
                                }
                            )]
            pub relayer_transaction: AccountInfo<'info>,

            pub system_program: Program<'info, System>,
        }
    };
    ($transaction:expr) => {
        transaction_pda_accounts!($transaction, relayer_discovery::TRANSACTION_PDA_SEED);
    };
}
