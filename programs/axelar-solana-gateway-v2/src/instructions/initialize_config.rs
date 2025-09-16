use crate::{
    state::config::GatewayConfig, u256::U256, GatewayError, InitialVerifierSet, RotationDelaySecs,
    VerifierSetEpoch, VerifierSetTracker,
};
use anchor_lang::prelude::{
    borsh::{BorshDeserialize, BorshSerialize},
    *,
};
use axelar_solana_gateway::seed_prefixes::{GATEWAY_SEED, VERIFIER_SET_TRACKER_SEED};

#[derive(Accounts)]
#[instruction(params: InitializeConfigInstruction)]
pub struct InitializeConfigAccounts<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub upgrade_authority: Signer<'info>,
    /// CHECK: correct program_data pda is given
    /// CHECK: upgrade authority in program_data matches the one passed as signer
    #[account(
            seeds = [crate::ID.as_ref()],
            bump,
            seeds::program = anchor_lang::solana_program::bpf_loader_upgradeable::ID,
            constraint = program_data.upgrade_authority_address == Some(upgrade_authority.key()) @ GatewayError::InvalidUpgradeAuthority
        )]
    pub program_data: Account<'info, ProgramData>,
    /// The gateway configuration PDA being initialized
    #[account(
            init,
            payer = payer,
            space = 8 + std::mem::size_of::<GatewayConfig>(),
            seeds = [GATEWAY_SEED],
            bump
        )]
    pub gateway_root_pda: Account<'info, GatewayConfig>,
    pub system_program: Program<'info, System>,
    #[account(
            init,
            payer = payer,
            space = 8 + std::mem::size_of::<VerifierSetTracker>(),
            seeds = [
                VERIFIER_SET_TRACKER_SEED,
                params.initial_verifier_set.hash.as_slice()
            ],
            bump
        )]
    pub verifier_set_tracker_pda: Account<'info, VerifierSetTracker>,
}

#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct InitializeConfigInstruction {
    _padding: u8,
    /// The domain separator, used as an input for hashing payloads.
    pub domain_separator: [u8; 32],
    /// initial verifier set
    pub initial_verifier_set: InitialVerifierSet,
    /// the minimum delay required between rotations
    pub minimum_rotation_delay: RotationDelaySecs,
    /// The gateway operator.
    pub operator: Pubkey,
    /// how many n epochs do we consider valid
    pub previous_verifier_retention: VerifierSetEpoch,
}

impl InitializeConfigInstruction {
    pub fn new(
        domain_separator: [u8; 32],
        initial_verifier_set: InitialVerifierSet,
        minimum_rotation_delay: RotationDelaySecs,
        operator: Pubkey,
        previous_verifier_retention: VerifierSetEpoch,
    ) -> Self {
        Self {
            _padding: 0,
            domain_separator,
            initial_verifier_set,
            minimum_rotation_delay,
            operator,
            previous_verifier_retention,
        }
    }
}

pub fn initialize_config_handler(
    ctx: Context<InitializeConfigAccounts>,
    params: InitializeConfigInstruction,
) -> Result<()> {
    msg!("initialize_config_handler");

    let config = &mut ctx.accounts.gateway_root_pda;

    // Initialize GatewayConfig (i.e. root pda) state
    config.current_epoch = U256::from(1);
    config.previous_verifier_set_retention = params.previous_verifier_retention;
    config.minimum_rotation_delay = params.minimum_rotation_delay;
    config.last_rotation_timestamp = Clock::get()?.unix_timestamp as u64;
    config.operator = params.operator;
    config.domain_separator = params.domain_separator;
    config.bump = ctx.bumps.gateway_root_pda;

    let set_tracker = &mut ctx.accounts.verifier_set_tracker_pda;

    // Initialize verifier set tracker pda state
    set_tracker.bump = ctx.bumps.verifier_set_tracker_pda;
    set_tracker.epoch = U256::from(1);
    set_tracker.verifier_set_hash = params.initial_verifier_set.hash;

    Ok(())
}
