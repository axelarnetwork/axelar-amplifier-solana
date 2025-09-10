use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace, PartialEq, Eq, Debug)]
pub struct InterchainTokenService {
    /// The address of the Axelar ITS Hub contract.
    #[max_len(45)]
    pub its_hub_address: String,

    /// Name of the chain ITS is running on.
    #[max_len(45)]
    pub chain_name: String,

    /// Whether the ITS is paused.
    pub paused: bool,

    /// Trusted chains
    // TODO(v2) maybe use HashSet or light hash set
    // https://github.com/Lightprotocol/light-protocol/blob/light-hash-set-v2.0.0/program-libs/hash-set/src/lib.rs
    #[max_len(100, 100)]
    pub trusted_chains: Vec<String>,

    /// The bump seed used to derive the PDA, ensuring the address is valid.
    pub bump: u8,
}

impl InterchainTokenService {
    pub const SEED_PREFIX: &'static [u8] = b"interchain-token-service";

    /// Create a new `InterchainTokenService` instance.
    #[must_use]
    pub fn new(bump: u8, chain_name: String, its_hub_address: String) -> Self {
        Self {
            its_hub_address,
            chain_name,
            paused: false,
            trusted_chains: Vec::new(),
            bump,
        }
    }

    /// Pauses the Interchain Token Service.
    pub fn pause(&mut self) {
        self.paused = true;
    }

    /// Unpauses the Interchain Token Service.
    pub fn unpause(&mut self) {
        self.paused = false;
    }

    /// Returns the bump used to derive the ITS PDA.
    #[must_use]
    pub const fn bump(&self) -> u8 {
        self.bump
    }

    //// Add a chain as trusted
    pub fn add_trusted_chain(&mut self, chain_id: String) {
        // Only add if not already present to avoid duplicates
        if !self.trusted_chains.iter().any(|chain| chain == &chain_id) {
            self.trusted_chains.push(chain_id);
        }
    }

    /// Remove a chain from trusted
    pub fn remove_trusted_chain(&mut self, chain_id: &str) {
        self.trusted_chains.retain(|chain| chain != chain_id);
    }

    /// Checks whether or not a given chain is trusted
    #[must_use]
    pub fn is_trusted_chain(&self, chain_id: &str) -> bool {
        self.trusted_chains.iter().any(|chain| chain == chain_id)
    }
}
