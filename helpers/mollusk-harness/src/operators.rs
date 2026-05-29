use std::collections::HashMap;

use anchor_lang::{Discriminator, InstructionData, ToAccountMetas};
use mollusk_svm::{
    result::{Check, InstructionResult},
    Mollusk, MolluskContext,
};
use solana_axelar_operators::{OperatorAccount, OperatorRegistry};
use solana_sdk::{
    account::Account, instruction::Instruction, native_token::LAMPORTS_PER_SOL, pubkey::Pubkey,
};

use crate::{ensure_default_sbf_out_dir, msg, TestHarness};

pub trait OperatorsSetup: TestHarness {
    fn payer(&self) -> Pubkey;
    fn owner(&self) -> Pubkey;

    fn registry(&self) -> Pubkey {
        OperatorRegistry::find_pda().0
    }

    fn operator_account(&self, operator: &Pubkey) -> Pubkey {
        OperatorAccount::find_pda(operator).0
    }

    fn ensure_operators_initialized(&self) {
        let registry = self.registry();
        if self.account_exists(&registry) {
            return;
        }

        self.ensure_account_exists_with_lamports(self.payer(), LAMPORTS_PER_SOL * 100);
        self.ensure_account_exists_with_lamports(self.owner(), LAMPORTS_PER_SOL * 100);

        let ix = Instruction {
            program_id: solana_axelar_operators::ID,
            accounts: solana_axelar_operators::accounts::Initialize {
                payer: self.payer(),
                owner: self.owner(),
                registry,
                system_program: solana_sdk_ids::system_program::ID,
            }
            .to_account_metas(None),
            data: solana_axelar_operators::instruction::Initialize {}.data(),
        };

        self.ctx().process_and_validate_instruction(
            &ix,
            &[
                Check::success(),
                Check::account(&registry)
                    .owner(&solana_axelar_operators::ID)
                    .rent_exempt()
                    .build(),
            ],
        );

        msg!("Operators registry initialized.");
    }

    fn add_operator(&self, operator: Pubkey) -> InstructionResult {
        self.add_operator_with_checks(
            operator,
            &[
                Check::success(),
                Check::account(&self.operator_account(&operator))
                    .owner(&solana_axelar_operators::ID)
                    .rent_exempt()
                    .build(),
            ],
        )
    }

    fn add_operator_with_checks(&self, operator: Pubkey, checks: &[Check]) -> InstructionResult {
        self.ensure_operators_initialized();
        self.ensure_account_exists_with_lamports(operator, LAMPORTS_PER_SOL * 100);

        let operator_account = self.operator_account(&operator);
        let ix = Instruction {
            program_id: solana_axelar_operators::ID,
            accounts: solana_axelar_operators::accounts::AddOperator {
                owner: self.owner(),
                operator_to_add: operator,
                registry: self.registry(),
                operator_account,
                system_program: solana_sdk_ids::system_program::ID,
            }
            .to_account_metas(None),
            data: solana_axelar_operators::instruction::AddOperator {}.data(),
        };

        self.ctx().process_and_validate_instruction(&ix, checks)
    }

    fn remove_operator(&self, operator: Pubkey) -> InstructionResult {
        let operator_account = self.operator_account(&operator);
        let owner_lamports = self
            .get_account(&self.owner())
            .expect("owner account should exist")
            .lamports;
        let operator_account_lamports = self
            .get_account(&operator_account)
            .expect("operator account should exist")
            .lamports;

        let ix = Instruction {
            program_id: solana_axelar_operators::ID,
            accounts: solana_axelar_operators::accounts::RemoveOperator {
                owner: self.owner(),
                operator_to_remove: operator,
                registry: self.registry(),
                operator_account,
            }
            .to_account_metas(None),
            data: solana_axelar_operators::instruction::RemoveOperator {}.data(),
        };

        self.ctx().process_and_validate_instruction(
            &ix,
            &[
                Check::success(),
                Check::account(&operator_account).closed().build(),
                Check::account(&self.owner())
                    .lamports(owner_lamports + operator_account_lamports)
                    .build(),
            ],
        )
    }

    fn remove_operator_with_checks(&self, operator: Pubkey, checks: &[Check]) -> InstructionResult {
        let ix = Instruction {
            program_id: solana_axelar_operators::ID,
            accounts: solana_axelar_operators::accounts::RemoveOperator {
                owner: self.owner(),
                operator_to_remove: operator,
                registry: self.registry(),
                operator_account: self.operator_account(&operator),
            }
            .to_account_metas(None),
            data: solana_axelar_operators::instruction::RemoveOperator {}.data(),
        };

        self.ctx().process_and_validate_instruction(&ix, checks)
    }

    fn transfer_owner(&self, new_owner: Pubkey) -> InstructionResult {
        self.ensure_account_exists_with_lamports(new_owner, LAMPORTS_PER_SOL * 100);

        let ix = Instruction {
            program_id: solana_axelar_operators::ID,
            accounts: solana_axelar_operators::accounts::TransferOwner {
                owner: self.owner(),
                new_owner,
                registry: self.registry(),
            }
            .to_account_metas(None),
            data: solana_axelar_operators::instruction::TransferOwner {}.data(),
        };

        self.ctx().process_and_validate_instruction(
            &ix,
            &[
                Check::success(),
                Check::account(&self.registry())
                    .data_slice(OperatorRegistry::DISCRIMINATOR.len(), new_owner.as_array())
                    .build(),
            ],
        )
    }

    fn transfer_owner_with_checks(&self, new_owner: Pubkey, checks: &[Check]) -> InstructionResult {
        self.ensure_account_exists_with_lamports(new_owner, LAMPORTS_PER_SOL * 100);

        let ix = Instruction {
            program_id: solana_axelar_operators::ID,
            accounts: solana_axelar_operators::accounts::TransferOwner {
                owner: self.owner(),
                new_owner,
                registry: self.registry(),
            }
            .to_account_metas(None),
            data: solana_axelar_operators::instruction::TransferOwner {}.data(),
        };

        self.ctx().process_and_validate_instruction(&ix, checks)
    }
}

pub struct OperatorsTestHarness {
    pub ctx: MolluskContext<HashMap<Pubkey, Account>>,
    pub payer: Pubkey,
    pub owner: Pubkey,
}

impl TestHarness for OperatorsTestHarness {
    fn ctx(&self) -> &MolluskContext<HashMap<Pubkey, Account>> {
        &self.ctx
    }
}

impl OperatorsSetup for OperatorsTestHarness {
    fn payer(&self) -> Pubkey {
        self.payer
    }

    fn owner(&self) -> Pubkey {
        self.owner
    }
}

impl Default for OperatorsTestHarness {
    fn default() -> Self {
        let mollusk = initialize_operators_mollusk();

        Self {
            ctx: mollusk.with_context(HashMap::new()),
            payer: Pubkey::new_unique(),
            owner: Pubkey::new_unique(),
        }
    }
}

impl OperatorsTestHarness {
    pub fn new() -> Self {
        let harness = Self::default();

        harness.ensure_operators_initialized();

        harness
    }
}

pub fn initialize_operators_mollusk() -> Mollusk {
    ensure_default_sbf_out_dir();
    Mollusk::new(&solana_axelar_operators::ID, "solana_axelar_operators")
}
