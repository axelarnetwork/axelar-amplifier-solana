use anchor_lang::prelude::*;

#[event]
pub struct TrustedChainSet {
    pub chain_name: String,
}

#[event]
pub struct TrustedChainRemoved {
    pub chain_name: String,
}

#[event]
pub struct InterchainTokenIdClaimed {
    pub token_id: [u8; 32],
    pub deployer: Pubkey,
    pub salt: [u8; 32],
}

#[event]
pub struct InterchainTokenDeployed {
    pub token_id: [u8; 32],
    pub token_address: Pubkey,
    pub minter: Pubkey,
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
}
