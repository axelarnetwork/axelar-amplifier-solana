use anchor_lang::InstructionData;
use axelar_solana_gas_service::instructions as v1_instructions;
use axelar_solana_gas_service_v2::instruction;

#[test]
fn test_v1_instructions_compatibility() {
    let v1_initialize = borsh::to_vec(&v1_instructions::GasServiceInstruction::Initialize).unwrap();
    let v2_initialize = instruction::Initialize {}.data();
    assert_eq!(v1_initialize, v2_initialize);

    let amount = 1000u64;
    let decimals = 8u8;
    let v1_collect_spl_fees =
        borsh::to_vec(&v1_instructions::GasServiceInstruction::CollectSplFees { amount, decimals })
            .unwrap();
    let v2_collect_spl_fees = instruction::CollectSplFees { amount, decimals }.data();
    assert_eq!(v1_collect_spl_fees, v2_collect_spl_fees);

    // TODO add rest of instructions
}
