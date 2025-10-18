use anchor_lang::AccountDeserialize;
use axelar_solana_governance_v2::{state::GovernanceConfig, GovernanceConfigUpdate};
use axelar_solana_governance_v2_test_fixtures::{
    initialize_governance,
    mock_setup_test, update_config,
};

#[test]
fn should_update_config() {
    let setup = mock_setup_test();
    let chain_hash = [1u8; 32];
    let address_hash = [2u8; 32];
    let minimum_proposal_eta_delay = 3600;

    let governance_config = GovernanceConfig::new(
        chain_hash,
        address_hash,
        minimum_proposal_eta_delay,
        setup.operator.to_bytes(),
    );

    let result = initialize_governance(&setup, governance_config);
    assert!(!result.program_result.is_err());

    let governance_config_account = result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == setup.governance_config)
        .unwrap()
        .1
        .clone();

    let new_chain_hash = [3u8; 32];
    let new_address_hash = [4u8; 32];
    let new_minimum_proposal_eta_delay = 7200;

    let params = GovernanceConfigUpdate {
        chain_hash: Some(new_chain_hash),
        address_hash: Some(new_address_hash),
        minimum_proposal_eta_delay: Some(new_minimum_proposal_eta_delay),
    };

    let result = update_config(&setup, params.clone(), governance_config_account.data);
    assert!(!result.program_result.is_err());

    let governance_config_account = result
        .resulting_accounts
        .iter()
        .find(|(pubkey, _)| *pubkey == setup.governance_config)
        .unwrap()
        .1
        .clone();

    let updated_config =
        GovernanceConfig::try_deserialize(&mut governance_config_account.data.as_slice()).unwrap();

    assert_eq!(updated_config.chain_hash, new_chain_hash);
}
