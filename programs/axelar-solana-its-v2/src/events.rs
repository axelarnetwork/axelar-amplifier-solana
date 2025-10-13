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

#[event]
pub struct InterchainTokenDeploymentStarted {
    pub token_id: [u8; 32],
    pub token_name: String,
    pub token_symbol: String,
    pub token_decimals: u8,
    pub minter: Vec<u8>,
    pub destination_chain: String,
}

#[event]
pub struct DeployRemoteInterchainTokenApproval {
    pub minter: Pubkey,
    pub deployer: Pubkey,
    pub token_id: [u8; 32],
    pub destination_chain: String,
    pub destination_minter: Vec<u8>,
}
