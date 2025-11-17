use anchor_lang::{InstructionData, ToAccountMetas};
use anchor_spl::token_2022::spl_token_2022;
use mollusk_svm::{
    program::keyed_account_for_system_program, result::Check, result::InstructionResult, Mollusk,
};
use mollusk_svm_programs_token;
use solana_axelar_its::state::{RoleProposal, UserRoles};
use solana_sdk::{account::Account, instruction::Instruction, pubkey::Pubkey};

use crate::new_empty_account;

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
            proposal_account: (proposal_pda, new_empty_account()),
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

pub struct AcceptOperatorshipContext {
    pub mollusk: Mollusk,
    pub payer: (Pubkey, Account),
    pub destination_user_account: (Pubkey, Account),
    pub destination_roles_account: (Pubkey, Account),
    pub resource_account: (Pubkey, Account),
    pub origin_user_account: (Pubkey, Account),
    pub origin_roles_account: (Pubkey, Account),
    pub proposal_account: (Pubkey, Account),
}

impl AcceptOperatorshipContext {
    pub fn new(
        mollusk: Mollusk,
        payer: (Pubkey, Account),
        destination_user_account: (Pubkey, Account),
        resource_account: (Pubkey, Account),
        origin_user_account: (Pubkey, Account),
        origin_roles_account: (Pubkey, Account),
        proposal_account: (Pubkey, Account),
    ) -> Self {
        let (destination_roles_pda, _bump) = solana_axelar_its::state::UserRoles::find_pda(
            &resource_account.0,
            &destination_user_account.0,
        );

        Self {
            mollusk,
            payer,
            destination_user_account,
            destination_roles_account: (destination_roles_pda, new_empty_account()),
            resource_account,
            origin_user_account,
            origin_roles_account,
            proposal_account,
        }
    }

    pub fn with_custom_destination_roles_account(
        mollusk: Mollusk,
        payer: (Pubkey, Account),
        destination_user_account: (Pubkey, Account),
        custom_destination_roles_account: (Pubkey, Account),
        resource_account: (Pubkey, Account),
        origin_user_account: (Pubkey, Account),
        origin_roles_account: (Pubkey, Account),
        proposal_account: (Pubkey, Account),
    ) -> Self {
        Self {
            mollusk,
            payer,
            destination_user_account,
            destination_roles_account: custom_destination_roles_account,
            resource_account,
            origin_user_account,
            origin_roles_account,
            proposal_account,
        }
    }
}

pub fn accept_operatorship_helper(
    ctx: AcceptOperatorshipContext,
    checks: Vec<Check>,
) -> (InstructionResult, Mollusk) {
    let program_id = solana_axelar_its::id();

    let ix = Instruction {
        program_id,
        accounts: solana_axelar_its::accounts::AcceptOperatorship {
            system_program: solana_sdk::system_program::ID,
            payer: ctx.payer.0,
            destination_user_account: ctx.destination_user_account.0,
            destination_roles_account: ctx.destination_roles_account.0,
            resource_account: ctx.resource_account.0,
            origin_user_account: ctx.origin_user_account.0,
            origin_roles_account: ctx.origin_roles_account.0,
            proposal_account: ctx.proposal_account.0,
        }
        .to_account_metas(None),
        data: solana_axelar_its::instruction::AcceptOperatorship {}.data(),
    };

    let accounts = vec![
        keyed_account_for_system_program(),
        ctx.payer,
        ctx.destination_user_account,
        ctx.destination_roles_account,
        ctx.resource_account,
        ctx.origin_user_account,
        ctx.origin_roles_account,
        ctx.proposal_account,
    ];

    let result = ctx
        .mollusk
        .process_and_validate_instruction(&ix, &accounts, &checks);
    (result, ctx.mollusk)
}

pub struct ProposeTokenManagerOperatorshipContext {
    pub mollusk: Mollusk,
    pub payer: (Pubkey, Account),
    pub origin_user_account: (Pubkey, Account),
    pub origin_roles_account: (Pubkey, Account),
    pub its_root_pda: (Pubkey, Account),
    pub token_manager_account: (Pubkey, Account),
    pub destination_user_account: (Pubkey, Account),
    pub proposal_account: (Pubkey, Account),
}

impl ProposeTokenManagerOperatorshipContext {
    pub fn new(
        mollusk: Mollusk,
        payer: (Pubkey, Account),
        origin_user_account: (Pubkey, Account),
        origin_roles_account: (Pubkey, Account),
        its_root_pda: (Pubkey, Account),
        token_manager_account: (Pubkey, Account),
        destination_user_account: (Pubkey, Account),
    ) -> Self {
        let program_id = solana_axelar_its::id();

        let (proposal_pda, _bump) = RoleProposal::find_pda(
            &token_manager_account.0,
            &origin_user_account.0,
            &destination_user_account.0,
            &program_id,
        );

        Self {
            mollusk,
            payer,
            origin_user_account,
            origin_roles_account,
            its_root_pda,
            token_manager_account,
            destination_user_account,
            proposal_account: (proposal_pda, new_empty_account()),
        }
    }

    pub fn with_custom_proposal_account(
        mollusk: Mollusk,
        payer: (Pubkey, Account),
        origin_user_account: (Pubkey, Account),
        origin_roles_account: (Pubkey, Account),
        its_root_pda: (Pubkey, Account),
        token_manager_account: (Pubkey, Account),
        destination_user_account: (Pubkey, Account),
        custom_proposal_account: (Pubkey, Account),
    ) -> Self {
        Self {
            mollusk,
            payer,
            origin_user_account,
            origin_roles_account,
            its_root_pda,
            token_manager_account,
            destination_user_account,
            proposal_account: custom_proposal_account,
        }
    }
}

pub fn propose_token_manager_operatorship_helper(
    ctx: ProposeTokenManagerOperatorshipContext,
    checks: Vec<Check>,
) -> (InstructionResult, Mollusk) {
    let program_id = solana_axelar_its::id();

    let ix = Instruction {
        program_id,
        accounts: solana_axelar_its::accounts::ProposeTokenManagerOperatorship {
            system_program: solana_sdk::system_program::ID,
            payer: ctx.payer.0,
            origin_user_account: ctx.origin_user_account.0,
            origin_roles_account: ctx.origin_roles_account.0,
            its_root_pda: ctx.its_root_pda.0,
            token_manager_account: ctx.token_manager_account.0,
            destination_user_account: ctx.destination_user_account.0,
            proposal_account: ctx.proposal_account.0,
        }
        .to_account_metas(None),
        data: solana_axelar_its::instruction::ProposeTokenManagerOperatorship {}.data(),
    };

    let accounts = vec![
        keyed_account_for_system_program(),
        ctx.payer,
        ctx.origin_user_account,
        ctx.origin_roles_account,
        ctx.its_root_pda,
        ctx.token_manager_account,
        ctx.destination_user_account,
        ctx.proposal_account,
    ];

    let result = ctx
        .mollusk
        .process_and_validate_instruction(&ix, &accounts, &checks);
    (result, ctx.mollusk)
}

pub struct AcceptTokenManagerOperatorshipContext {
    pub mollusk: Mollusk,
    pub payer: (Pubkey, Account),
    pub destination_user_account: (Pubkey, Account),
    pub destination_roles_account: (Pubkey, Account),
    pub its_root_pda: (Pubkey, Account),
    pub token_manager_account: (Pubkey, Account),
    pub origin_user_account: (Pubkey, Account),
    pub origin_roles_account: (Pubkey, Account),
    pub proposal_account: (Pubkey, Account),
}

impl AcceptTokenManagerOperatorshipContext {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        mollusk: Mollusk,
        payer: (Pubkey, Account),
        destination_user_account: (Pubkey, Account),
        destination_roles_pda: Pubkey,
        its_root_pda: (Pubkey, Account),
        token_manager_account: (Pubkey, Account),
        origin_user_account: (Pubkey, Account),
        origin_roles_account: (Pubkey, Account),
        proposal_account: (Pubkey, Account),
    ) -> Self {
        Self {
            mollusk,
            payer,
            destination_user_account,
            destination_roles_account: (destination_roles_pda, new_empty_account()),
            its_root_pda,
            token_manager_account,
            origin_user_account,
            origin_roles_account,
            proposal_account,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn with_custom_destination_roles_account(
        mollusk: Mollusk,
        payer: (Pubkey, Account),
        destination_user_account: (Pubkey, Account),
        custom_destination_roles_account: (Pubkey, Account),
        its_root_pda: (Pubkey, Account),
        token_manager_account: (Pubkey, Account),
        origin_user_account: (Pubkey, Account),
        origin_roles_account: (Pubkey, Account),
        proposal_account: (Pubkey, Account),
    ) -> Self {
        Self {
            mollusk,
            payer,
            destination_user_account,
            destination_roles_account: custom_destination_roles_account,
            its_root_pda,
            token_manager_account,
            origin_user_account,
            origin_roles_account,
            proposal_account,
        }
    }
}

pub fn accept_token_manager_operatorship_helper(
    ctx: AcceptTokenManagerOperatorshipContext,
    checks: Vec<Check>,
) -> (InstructionResult, Mollusk) {
    let program_id = solana_axelar_its::id();

    let ix = Instruction {
        program_id,
        accounts: solana_axelar_its::accounts::AcceptTokenManagerOperatorship {
            system_program: solana_sdk::system_program::ID,
            payer: ctx.payer.0,
            destination_user_account: ctx.destination_user_account.0,
            destination_roles_account: ctx.destination_roles_account.0,
            its_root_pda: ctx.its_root_pda.0,
            token_manager_account: ctx.token_manager_account.0,
            origin_user_account: ctx.origin_user_account.0,
            origin_roles_account: ctx.origin_roles_account.0,
            proposal_account: ctx.proposal_account.0,
        }
        .to_account_metas(None),
        data: solana_axelar_its::instruction::AcceptTokenManagerOperatorship {}.data(),
    };

    let accounts = vec![
        keyed_account_for_system_program(),
        ctx.payer,
        ctx.destination_user_account,
        ctx.destination_roles_account,
        ctx.its_root_pda,
        ctx.token_manager_account,
        ctx.origin_user_account,
        ctx.origin_roles_account,
        ctx.proposal_account,
    ];

    let result = ctx
        .mollusk
        .process_and_validate_instruction(&ix, &accounts, &checks);
    (result, ctx.mollusk)
}

pub struct ProposeInterchainTokenMintershipContext {
    pub mollusk: Mollusk,
    pub payer: (Pubkey, Account),
    pub origin_user_account: (Pubkey, Account),
    pub origin_roles_account: (Pubkey, Account),
    pub its_root_pda: (Pubkey, Account),
    pub token_manager_account: (Pubkey, Account),
    pub destination_user_account: (Pubkey, Account),
    pub proposal_account: (Pubkey, Account),
}

impl ProposeInterchainTokenMintershipContext {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        mollusk: Mollusk,
        payer: (Pubkey, Account),
        origin_user_account: (Pubkey, Account),
        origin_roles_account: (Pubkey, Account),
        its_root_pda: (Pubkey, Account),
        token_manager_account: (Pubkey, Account),
        destination_user_account: (Pubkey, Account),
    ) -> Self {
        let program_id = solana_axelar_its::id();

        let (proposal_pda, _bump) = RoleProposal::find_pda(
            &token_manager_account.0,
            &origin_user_account.0,
            &destination_user_account.0,
            &program_id,
        );

        Self {
            mollusk,
            payer,
            origin_user_account,
            origin_roles_account,
            its_root_pda,
            token_manager_account,
            destination_user_account,
            proposal_account: (proposal_pda, new_empty_account()),
        }
    }
}

pub fn propose_interchain_token_mintership_helper(
    ctx: ProposeInterchainTokenMintershipContext,
    checks: Vec<Check>,
) -> (InstructionResult, Mollusk) {
    let program_id = solana_axelar_its::id();

    let ix = Instruction {
        program_id,
        accounts: solana_axelar_its::accounts::ProposeInterchainTokenMintership {
            system_program: solana_sdk::system_program::ID,
            payer: ctx.payer.0,
            origin_user_account: ctx.origin_user_account.0,
            origin_roles_account: ctx.origin_roles_account.0,
            its_root_pda: ctx.its_root_pda.0,
            token_manager_account: ctx.token_manager_account.0,
            destination_user_account: ctx.destination_user_account.0,
            proposal_account: ctx.proposal_account.0,
        }
        .to_account_metas(None),
        data: solana_axelar_its::instruction::ProposeInterchainTokenMintership {}.data(),
    };

    let accounts = vec![
        keyed_account_for_system_program(),
        ctx.payer,
        ctx.origin_user_account,
        ctx.origin_roles_account,
        ctx.its_root_pda,
        ctx.token_manager_account,
        ctx.destination_user_account,
        ctx.proposal_account,
    ];

    let result = ctx
        .mollusk
        .process_and_validate_instruction(&ix, &accounts, &checks);
    (result, ctx.mollusk)
}

pub struct AcceptInterchainTokenMintershipContext {
    pub mollusk: Mollusk,
    pub payer: (Pubkey, Account),
    pub destination_user_account: (Pubkey, Account),
    pub destination_roles_account: (Pubkey, Account),
    pub its_root_pda: (Pubkey, Account),
    pub token_manager_account: (Pubkey, Account),
    pub origin_user_account: (Pubkey, Account),
    pub origin_roles_account: (Pubkey, Account),
    pub proposal_account: (Pubkey, Account),
}

impl AcceptInterchainTokenMintershipContext {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        mollusk: Mollusk,
        payer: (Pubkey, Account),
        destination_user_account: (Pubkey, Account),
        destination_roles_pda: Pubkey,
        its_root_pda: (Pubkey, Account),
        token_manager_account: (Pubkey, Account),
        origin_user_account: (Pubkey, Account),
        origin_roles_account: (Pubkey, Account),
        proposal_account: (Pubkey, Account),
    ) -> Self {
        Self {
            mollusk,
            payer,
            destination_user_account,
            destination_roles_account: (destination_roles_pda, new_empty_account()),
            its_root_pda,
            token_manager_account,
            origin_user_account,
            origin_roles_account,
            proposal_account,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn with_custom_destination_roles_account(
        mollusk: Mollusk,
        payer: (Pubkey, Account),
        destination_user_account: (Pubkey, Account),
        custom_destination_roles_account: (Pubkey, Account),
        its_root_pda: (Pubkey, Account),
        token_manager_account: (Pubkey, Account),
        origin_user_account: (Pubkey, Account),
        origin_roles_account: (Pubkey, Account),
        proposal_account: (Pubkey, Account),
    ) -> Self {
        Self {
            mollusk,
            payer,
            destination_user_account,
            destination_roles_account: custom_destination_roles_account,
            its_root_pda,
            token_manager_account,
            origin_user_account,
            origin_roles_account,
            proposal_account,
        }
    }
}

pub fn accept_interchain_token_mintership_helper(
    ctx: AcceptInterchainTokenMintershipContext,
    checks: Vec<Check>,
) -> (InstructionResult, Mollusk) {
    let program_id = solana_axelar_its::id();

    let ix = Instruction {
        program_id,
        accounts: solana_axelar_its::accounts::AcceptInterchainTokenMintership {
            system_program: solana_sdk::system_program::ID,
            payer: ctx.payer.0,
            destination_user_account: ctx.destination_user_account.0,
            destination_roles_account: ctx.destination_roles_account.0,
            its_root_pda: ctx.its_root_pda.0,
            token_manager_account: ctx.token_manager_account.0,
            origin_user_account: ctx.origin_user_account.0,
            origin_roles_account: ctx.origin_roles_account.0,
            proposal_account: ctx.proposal_account.0,
        }
        .to_account_metas(None),
        data: solana_axelar_its::instruction::AcceptInterchainTokenMintership {}.data(),
    };

    let accounts = vec![
        keyed_account_for_system_program(),
        ctx.payer,
        ctx.destination_user_account,
        ctx.destination_roles_account,
        ctx.its_root_pda,
        ctx.token_manager_account,
        ctx.origin_user_account,
        ctx.origin_roles_account,
        ctx.proposal_account,
    ];

    let result = ctx
        .mollusk
        .process_and_validate_instruction(&ix, &accounts, &checks);
    (result, ctx.mollusk)
}

pub struct HandoverMintAuthorityContext {
    pub mollusk: Mollusk,
    pub payer: (Pubkey, Account),
    pub authority: (Pubkey, Account),
    pub mint: (Pubkey, Account),
    pub its_root: (Pubkey, Account),
    pub token_manager: (Pubkey, Account),
    pub minter_roles: (Pubkey, Account),
    pub token_id: [u8; 32],
}

impl HandoverMintAuthorityContext {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        mollusk: Mollusk,
        payer: (Pubkey, Account),
        authority: (Pubkey, Account),
        mint: (Pubkey, Account),
        its_root: (Pubkey, Account),
        token_manager: (Pubkey, Account),
        token_id: [u8; 32],
    ) -> Self {
        let (minter_roles_pda, _) = UserRoles::find_pda(&token_manager.0, &authority.0);

        Self {
            mollusk,
            payer,
            authority,
            mint,
            its_root,
            token_manager,
            minter_roles: (minter_roles_pda, new_empty_account()),
            token_id,
        }
    }

    pub fn with_custom_minter_roles_account(mut self, minter_roles_account: Account) -> Self {
        self.minter_roles.1 = minter_roles_account;
        self
    }
}

pub fn handover_mint_authority_helper(
    ctx: HandoverMintAuthorityContext,
    checks: Vec<Check>,
) -> (InstructionResult, Mollusk) {
    let program_id = solana_axelar_its::id();

    let ix = Instruction {
        program_id,
        accounts: solana_axelar_its::accounts::HandoverMintAuthority {
            payer: ctx.payer.0,
            authority: ctx.authority.0,
            mint: ctx.mint.0,
            its_root: ctx.its_root.0,
            token_manager: ctx.token_manager.0,
            minter_roles: ctx.minter_roles.0,
            token_program: spl_token_2022::ID,
            system_program: solana_sdk::system_program::ID,
        }
        .to_account_metas(None),
        data: solana_axelar_its::instruction::HandoverMintAuthority {
            token_id: ctx.token_id,
        }
        .data(),
    };

    let accounts = vec![
        ctx.payer,
        ctx.authority,
        ctx.mint,
        ctx.its_root,
        ctx.token_manager,
        ctx.minter_roles,
        mollusk_svm_programs_token::token2022::keyed_account(),
        keyed_account_for_system_program(),
    ];

    let result = ctx
        .mollusk
        .process_and_validate_instruction(&ix, &accounts, &checks);
    (result, ctx.mollusk)
}
