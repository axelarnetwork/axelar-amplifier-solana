use anchor_lang::{InstructionData, ToAccountMetas};
use mollusk_svm::{
    program::keyed_account_for_system_program, result::Check, result::InstructionResult, Mollusk,
};
use solana_axelar_its::state::RoleProposal;
use solana_sdk::{account::Account, instruction::Instruction, pubkey::Pubkey};

pub struct ProposeOperatorshipContext {
    pub mollusk: Mollusk,
    pub payer: (Pubkey, Account),
    pub origin_user_account: (Pubkey, Account),
    pub origin_roles_account: (Pubkey, Account),
    pub resource_account: (Pubkey, Account),
    pub destination_user_account: (Pubkey, Account),
    pub proposal_account: (Pubkey, Account),
}

impl ProposeOperatorshipContext {
    pub fn new(
        mollusk: Mollusk,
        payer: (Pubkey, Account),
        origin_user_account: (Pubkey, Account),
        origin_roles_account: (Pubkey, Account),
        resource_account: (Pubkey, Account),
        destination_user_account: (Pubkey, Account),
    ) -> Self {
        let program_id = solana_axelar_its::id();

        let (proposal_pda, _bump) = RoleProposal::find_pda(
            &resource_account.0,
            &origin_user_account.0,
            &destination_user_account.0,
            &program_id,
        );

        Self {
            mollusk,
            payer,
            origin_user_account,
            origin_roles_account,
            resource_account,
            destination_user_account,
            proposal_account: (
                proposal_pda,
                Account::new(0, 0, &solana_sdk::system_program::ID),
            ),
        }
    }
}

pub fn propose_operatorship_helper(
    ctx: ProposeOperatorshipContext,
    checks: Vec<Check>,
) -> (InstructionResult, Mollusk) {
    let program_id = solana_axelar_its::id();

    let ix = Instruction {
        program_id,
        accounts: solana_axelar_its::accounts::ProposeOperatorship {
            system_program: solana_sdk::system_program::ID,
            payer: ctx.payer.0,
            origin_user_account: ctx.origin_user_account.0,
            origin_roles_account: ctx.origin_roles_account.0,
            resource_account: ctx.resource_account.0,
            destination_user_account: ctx.destination_user_account.0,
            proposal_account: ctx.proposal_account.0,
        }
        .to_account_metas(None),
        data: solana_axelar_its::instruction::ProposeOperatorship {}.data(),
    };

    let accounts = vec![
        keyed_account_for_system_program(),
        ctx.payer,
        ctx.origin_user_account,
        ctx.origin_roles_account,
        ctx.resource_account,
        ctx.destination_user_account,
        ctx.proposal_account,
    ];

    let result = ctx
        .mollusk
        .process_and_validate_instruction(&ix, &accounts, &checks);
    (result, ctx.mollusk)
}
