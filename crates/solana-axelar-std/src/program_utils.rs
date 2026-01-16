/// Macro to ensure exactly one feature from a list is enabled
#[macro_export]
macro_rules! ensure_single_feature {
    ($($feature:literal),+) => {
        // Check that at least one feature is enabled
        #[cfg(not(any($(feature = $feature),+)))]
        compile_error!(concat!("Exactly one of these features must be enabled: ", $(stringify!($feature), ", "),+));

        // Generate all pair combinations to check mutual exclusivity
        ensure_single_feature!(@pairs [] $($feature),+);
    };

    // Helper to generate all pairs
    (@pairs [$($processed:literal),*] $first:literal $(,$rest:literal)*) => {
        // Check current element against all processed elements
        $(
            #[cfg(all(feature = $first, feature = $processed))]
            compile_error!(concat!("Features '", $first, "' and '", $processed, "' are mutually exclusive"));
        )*

        // Continue with the rest
        ensure_single_feature!(@pairs [$($processed,)* $first] $($rest),*);
    };

    // Base case: no more elements to process
    (@pairs [$($processed:literal),*]) => {};
}

/// Macro for transferring lamports between accounts that implement the Lamports trait
///
/// # Requirements
///
/// 1. The `from` account must be owned by the executing program.
/// 2. Both accounts must be marked `mut`.
/// 3. The total lamports **before** the transaction must equal to total lamports **after**
///    the transaction.
/// 4. `lamports` field of both account infos should not currently be borrowed.
///
/// # Examples
/// ```ignore
/// transfer_lamports_anchor!(
///  ctx.accounts.from_account,
///  ctx.accounts.to_account,
///  amount,
/// );
/// ```
#[macro_export]
macro_rules! transfer_lamports_anchor {
    ($from:expr, $to:expr, $amount:expr) => {{
        if $from.get_lamports() < $amount {
            return Err(anchor_lang::error::Error::from(
                anchor_lang::solana_program::program_error::ProgramError::InsufficientFunds,
            ));
        }
        $from.sub_lamports($amount)?;
        $to.add_lamports($amount)?;
    }};
}
