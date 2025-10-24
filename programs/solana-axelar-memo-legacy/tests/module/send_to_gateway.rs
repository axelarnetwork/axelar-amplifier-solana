use ethers_core::abi::AbiEncode;
use event_cpi_test_utils::assert_event_cpi;
use solana_axelar_gateway_legacy::events::CallContractEvent;
use solana_axelar_memo_legacy::get_counter_pda;
use solana_axelar_memo_legacy::instruction::call_gateway_with_memo;
use solana_program_test::tokio;
use solana_sdk::signer::Signer;

use crate::program_test;

#[tokio::test]
async fn test_successfully_send_to_gateway() {
    // Setup
    let mut solana_chain = program_test().await;
    let memo = "🐪🐪🐪🐪";
    let destination_address = ethers_core::types::Address::random().encode_hex();
    let destination_chain = "ethereum".to_string();
    let (counter_pda, counter_bump) = get_counter_pda();
    let initialize = solana_axelar_memo_legacy::instruction::initialize(
        &solana_chain.fixture.payer.pubkey().clone(),
        &(counter_pda, counter_bump),
    )
    .unwrap();
    solana_chain.send_tx(&[initialize]).await.unwrap();

    // Action: send message to gateway
    let call_gateway_with_memo = call_gateway_with_memo(
        &solana_chain.gateway_root_pda,
        &counter_pda,
        memo.to_string(),
        destination_chain.clone(),
        destination_address.clone(),
        &solana_axelar_gateway_legacy::ID,
    )
    .unwrap();
    let simulate_tx = solana_chain
        .simulate_tx(&[call_gateway_with_memo.clone()])
        .await
        .unwrap();
    let inner_ixs = simulate_tx
        .simulation_details
        .unwrap()
        .inner_instructions
        .unwrap()
        .first()
        .cloned()
        .unwrap();
    // Assert
    // We can get the memo from the logs
    assert_event_cpi(
        &CallContractEvent {
            sender: solana_axelar_memo_legacy::ID,
            destination_chain,
            destination_contract_address: destination_address,
            payload: memo.as_bytes().to_vec(),
            payload_hash: solana_sdk::keccak::hash(memo.as_bytes()).0,
        },
        &inner_ixs,
    );

    let _tx = solana_chain
        .send_tx(&[call_gateway_with_memo])
        .await
        .unwrap();
}
