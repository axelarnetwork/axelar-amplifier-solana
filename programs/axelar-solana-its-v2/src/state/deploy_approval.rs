use anchor_lang::prelude::*;

use crate::utils::interchain_token_id;

#[account]
#[derive(InitSpace, Debug)]
pub struct DeployApproval {
    /// Hash of the approved destination minter
    pub approved_destination_minter: [u8; 32],
    /// PDA bump seed
    pub bump: u8,
}

impl DeployApproval {
    pub fn find_pda(
        minter: &Pubkey,
        deployer: &Pubkey,
        salt: &[u8; 32],
        destination_chain: &str,
    ) -> (Pubkey, u8) {
        let token_id = interchain_token_id(deployer, salt);
        let destination_chain_hash =
            anchor_lang::solana_program::keccak::hashv(&[destination_chain.as_bytes()]).to_bytes();

        Pubkey::find_program_address(
            &[
                crate::seed_prefixes::DEPLOYMENT_APPROVAL_SEED,
                minter.as_ref(),
                &token_id,
                &destination_chain_hash,
            ],
            &crate::ID,
        )
    }
}
