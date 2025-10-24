use axelar_solana_gateway_test_fixtures::{
    SolanaAxelarIntegration, SolanaAxelarIntegrationMetadata,
};

mod initialize;
mod send_to_gateway;
mod validate_message;

pub async fn program_test() -> SolanaAxelarIntegrationMetadata {
    SolanaAxelarIntegration::builder()
        .initial_signer_weights(vec![555, 222])
        .programs_to_deploy(vec![(
            "axelar_solana_memo_legacy.so".into(),
            axelar_solana_memo_legacy::id(),
        )])
        .build()
        .setup()
        .await
}
