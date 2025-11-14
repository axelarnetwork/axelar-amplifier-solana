use anchor_lang::prelude::*;

/// A counter PDA that keeps track of how many memos have been received from the
/// gateway
#[derive(Clone, Debug, PartialEq, AnchorSerialize, AnchorDeserialize)]
pub struct Payload {
    /// the counter of how many memos have been received from the gateway
    pub storage_id: u64,
    pub memo: String,
}
