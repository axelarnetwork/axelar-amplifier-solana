use anchor_lang::prelude::*;
use axelar_solana_gas_service::instructions::*;
use axelar_solana_gas_service_v2::GasServiceDiscriminators;

#[test]
fn test_discriminators_backwards_compatible() {
    // Test simple instructions
    let payer = Pubkey::new_unique();
    let operator = Pubkey::new_unique();

    let init_ix = init_config(&payer, &operator).unwrap();
    assert_eq!(&init_ix.data[0..1], GasServiceDiscriminators::INITIALIZE);
    println!("✓ Initialize: {:?}", &init_ix.data[0..1]);

    let transfer_ix = transfer_operatorship(&operator, &payer).unwrap();
    assert_eq!(
        &transfer_ix.data[0..1],
        GasServiceDiscriminators::TRANSFER_OPERATORSHIP
    );
    println!("✓ TransferOperatorship: {:?}", &transfer_ix.data[0..1]);

    // Test Native Token instructions
    let native_pay_ix = pay_native_for_contract_call_instruction(
        &payer,
        "ethereum".to_string(),
        "0x123".to_string(),
        [0u8; 32],
        Pubkey::default(),
        vec![],
        1000,
    )
    .unwrap();
    assert_eq!(
        &native_pay_ix.data[0..2],
        GasServiceDiscriminators::NATIVE_PAY_FOR_CONTRACT_CALL
    );
    println!(
        "✓ Native PayForContractCall: {:?}",
        &native_pay_ix.data[0..2]
    );

    let native_add_gas_ix =
        add_native_gas_instruction(&payer, [0u8; 64], 0, 500, Pubkey::default()).unwrap();
    assert_eq!(
        &native_add_gas_ix.data[0..2],
        GasServiceDiscriminators::NATIVE_ADD_GAS
    );
    println!("✓ Native AddGas: {:?}", &native_add_gas_ix.data[0..2]);

    let native_collect_ix = collect_native_fees_instruction(&operator, &payer, 100).unwrap();
    assert_eq!(
        &native_collect_ix.data[0..2],
        GasServiceDiscriminators::NATIVE_COLLECT_FEES
    );
    println!("✓ Native CollectFees: {:?}", &native_collect_ix.data[0..2]);

    let native_refund_ix =
        refund_native_fees_instruction(&operator, &payer, [1u8; 64], 1, 200).unwrap();
    assert_eq!(
        &native_refund_ix.data[0..2],
        GasServiceDiscriminators::NATIVE_REFUND
    );
    println!("✓ Native Refund: {:?}", &native_refund_ix.data[0..2]);

    // Test SPL Token instructions
    let mint = Pubkey::new_unique();
    let sender_ata = Pubkey::new_unique();
    let token_program_id = spl_token::ID;

    let spl_pay_ix = pay_spl_for_contract_call_instruction(
        &payer,
        &sender_ata,
        &mint,
        &token_program_id,
        "ethereum".to_string(),
        "0x456".to_string(),
        [1u8; 32],
        Pubkey::default(),
        vec![1, 2, 3],
        2000,
        &[],
        6,
    )
    .unwrap();
    assert_eq!(
        &spl_pay_ix.data[0..2],
        GasServiceDiscriminators::SPL_PAY_FOR_CONTRACT_CALL
    );
    println!("✓ SPL PayForContractCall: {:?}", &spl_pay_ix.data[0..2]);

    let spl_add_gas_ix = add_spl_gas_instruction(
        &payer,
        &sender_ata,
        &mint,
        &token_program_id,
        &[],
        [2u8; 64],
        2,
        1500,
        Pubkey::default(),
        6,
    )
    .unwrap();
    assert_eq!(
        &spl_add_gas_ix.data[0..2],
        GasServiceDiscriminators::SPL_ADD_GAS
    );
    println!("✓ SPL AddGas: {:?}", &spl_add_gas_ix.data[0..2]);

    let spl_collect_ix =
        collect_spl_fees_instruction(&operator, &token_program_id, &mint, &payer, 300, 6).unwrap();
    assert_eq!(
        &spl_collect_ix.data[0..2],
        GasServiceDiscriminators::SPL_COLLECT_FEES
    );
    println!("✓ SPL CollectFees: {:?}", &spl_collect_ix.data[0..2]);

    let spl_refund_ix = refund_spl_fees_instruction(
        &operator,
        &token_program_id,
        &mint,
        &payer,
        [3u8; 64],
        3,
        400,
        6,
    )
    .unwrap();
    assert_eq!(
        &spl_refund_ix.data[0..2],
        GasServiceDiscriminators::SPL_REFUND
    );
    println!("✓ SPL Refund: {:?}", &spl_refund_ix.data[0..2]);

    println!("🎉 All discriminators validated using helper functions!");
}
