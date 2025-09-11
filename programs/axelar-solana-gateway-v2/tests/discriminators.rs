use axelar_solana_gateway::instructions::GatewayInstruction;
use axelar_solana_gateway_v2::GatewayDiscriminators;

#[test]
fn test_call_contract_discriminator_backwards_compatible() -> Result<(), Box<dyn std::error::Error>>
{
    let destination_chain = "ethereum".to_string();
    let destination_contract_address = "0x1234567890123456789012345678901234567890".to_string();
    let payload = vec![1, 2, 3, 4, 5];
    let signing_pda_bump = 250u8;

    let v1_instruction = GatewayInstruction::CallContract {
        destination_chain,
        destination_contract_address,
        payload,
        signing_pda_bump,
    };

    let data = borsh::to_vec(&v1_instruction)?;

    let v1_discriminator = &data[0..1];

    // Compare with our v2 discriminator
    assert_eq!(
        v1_discriminator,
        GatewayDiscriminators::CALL_CONTRACT,
        "CallContract discriminator mismatch! V1: {:?}, V2: {:?}",
        v1_discriminator,
        GatewayDiscriminators::CALL_CONTRACT
    );

    Ok(())
}
