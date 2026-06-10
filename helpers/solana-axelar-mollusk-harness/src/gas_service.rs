use std::collections::HashMap;

use anchor_lang::{InstructionData, ToAccountMetas};
use mollusk_svm::{
    result::{Check, InstructionResult},
    Mollusk, MolluskContext,
};
use solana_axelar_gas_service::Treasury;
use solana_sdk::{
    account::Account, instruction::Instruction, native_token::LAMPORTS_PER_SOL, pubkey::Pubkey,
};

use crate::operators::OperatorsSetup;
use crate::{
    deployed_program_path, ensure_default_sbf_out_dir, get_event_authority_and_program_accounts,
    msg, TestHarness,
};

pub trait GasServiceSetup: OperatorsSetup {
    fn operator(&self) -> Pubkey;

    fn treasury(&self) -> Pubkey {
        Treasury::find_pda().0
    }

    fn ensure_gas_service_initialized(&self) {
        let treasury = self.treasury();
        if self.account_exists(&treasury) {
            return;
        }

        self.ensure_operators_initialized();

        let operator_pda = self.operator_account(&self.operator());
        if !self.account_exists(&operator_pda) {
            self.add_operator(self.operator());
        }

        let ix = Instruction {
            program_id: solana_axelar_gas_service::ID,
            accounts: solana_axelar_gas_service::accounts::Initialize {
                payer: self.operator(),
                operator: self.operator(),
                operator_pda,
                system_program: solana_sdk_ids::system_program::ID,
                treasury,
            }
            .to_account_metas(None),
            data: solana_axelar_gas_service::instruction::Initialize {}.data(),
        };

        self.ctx().process_and_validate_instruction(
            &ix,
            &[
                Check::success(),
                Check::account(&treasury)
                    .owner(&solana_axelar_gas_service::ID)
                    .rent_exempt()
                    .build(),
            ],
        );

        msg!("Gas service initialized.");
    }

    fn add_gas_ix(
        &self,
        sender: Pubkey,
        message_id: String,
        amount: u64,
        refund_address: Pubkey,
    ) -> Instruction {
        let (event_authority, _, _) =
            get_event_authority_and_program_accounts(&solana_axelar_gas_service::ID);

        Instruction {
            program_id: solana_axelar_gas_service::ID,
            accounts: solana_axelar_gas_service::accounts::AddGas {
                sender,
                treasury: self.treasury(),
                system_program: solana_sdk_ids::system_program::ID,
                event_authority,
                program: solana_axelar_gas_service::ID,
            }
            .to_account_metas(None),
            data: solana_axelar_gas_service::instruction::AddGas {
                message_id,
                amount,
                refund_address,
            }
            .data(),
        }
    }

    fn pay_gas_ix(
        &self,
        sender: Pubkey,
        destination_chain: String,
        destination_address: String,
        payload_hash: [u8; 32],
        amount: u64,
        refund_address: Pubkey,
    ) -> Instruction {
        let (event_authority, _, _) =
            get_event_authority_and_program_accounts(&solana_axelar_gas_service::ID);

        Instruction {
            program_id: solana_axelar_gas_service::ID,
            accounts: solana_axelar_gas_service::accounts::PayGas {
                sender,
                treasury: self.treasury(),
                system_program: solana_sdk_ids::system_program::ID,
                event_authority,
                program: solana_axelar_gas_service::ID,
            }
            .to_account_metas(None),
            data: solana_axelar_gas_service::instruction::PayGas {
                destination_chain,
                destination_address,
                payload_hash,
                amount,
                refund_address,
            }
            .data(),
        }
    }

    fn collect_fees_ix(&self, receiver: Pubkey, amount: u64) -> Instruction {
        let (event_authority, _, _) =
            get_event_authority_and_program_accounts(&solana_axelar_gas_service::ID);

        Instruction {
            program_id: solana_axelar_gas_service::ID,
            accounts: solana_axelar_gas_service::accounts::CollectFees {
                operator: self.operator(),
                operator_pda: self.operator_account(&self.operator()),
                receiver,
                treasury: self.treasury(),
                event_authority,
                program: solana_axelar_gas_service::ID,
            }
            .to_account_metas(None),
            data: solana_axelar_gas_service::instruction::CollectFees { amount }.data(),
        }
    }

    fn refund_fees_ix(&self, receiver: Pubkey, message_id: String, amount: u64) -> Instruction {
        let (event_authority, _, _) =
            get_event_authority_and_program_accounts(&solana_axelar_gas_service::ID);

        Instruction {
            program_id: solana_axelar_gas_service::ID,
            accounts: solana_axelar_gas_service::accounts::RefundFees {
                operator: self.operator(),
                operator_pda: self.operator_account(&self.operator()),
                receiver,
                treasury: self.treasury(),
                event_authority,
                program: solana_axelar_gas_service::ID,
            }
            .to_account_metas(None),
            data: solana_axelar_gas_service::instruction::RefundFees { message_id, amount }.data(),
        }
    }

    fn process_add_gas(
        &self,
        sender: Pubkey,
        message_id: String,
        amount: u64,
        refund_address: Pubkey,
        checks: &[Check],
    ) -> InstructionResult {
        let ix = self.add_gas_ix(sender, message_id, amount, refund_address);
        self.ctx().process_and_validate_instruction(&ix, checks)
    }
}

pub struct GasServiceTestHarness {
    pub ctx: MolluskContext<HashMap<Pubkey, Account>>,
    pub payer: Pubkey,
    pub operator: Pubkey,
}

impl TestHarness for GasServiceTestHarness {
    fn ctx(&self) -> &MolluskContext<HashMap<Pubkey, Account>> {
        &self.ctx
    }
}

impl OperatorsSetup for GasServiceTestHarness {
    fn payer(&self) -> Pubkey {
        self.payer
    }

    fn owner(&self) -> Pubkey {
        self.operator
    }
}

impl GasServiceSetup for GasServiceTestHarness {
    fn operator(&self) -> Pubkey {
        self.operator
    }
}

impl Default for GasServiceTestHarness {
    fn default() -> Self {
        let mollusk = initialize_gas_service_mollusk();

        Self {
            ctx: mollusk.with_context(HashMap::new()),
            payer: Pubkey::new_unique(),
            operator: Pubkey::new_unique(),
        }
    }
}

impl GasServiceTestHarness {
    pub fn new() -> Self {
        let harness = Self::default();

        harness.ensure_account_exists_with_lamports(harness.payer, LAMPORTS_PER_SOL * 100);
        harness.ensure_account_exists_with_lamports(harness.operator, LAMPORTS_PER_SOL * 100);
        harness.ensure_gas_service_initialized();

        harness
    }

    pub fn fund_treasury(&mut self, lamports: u64) {
        self.update_account(&self.treasury(), |account| {
            account.lamports += lamports;
        });
    }
}

pub fn initialize_gas_service_mollusk() -> Mollusk {
    ensure_default_sbf_out_dir();
    let mut mollusk = Mollusk::new(&solana_axelar_gas_service::ID, "solana_axelar_gas_service");

    let operators_program = deployed_program_path("solana_axelar_operators");
    mollusk.add_program(&solana_axelar_operators::ID, &operators_program);

    mollusk
}
