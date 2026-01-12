use anchor_lang::{InstructionData, ToAccountMetas};
use anchor_spl::token_2022::spl_token_2022;
use mollusk_svm::{
    program::keyed_account_for_system_program, result::Check, result::InstructionResult, Mollusk,
};
use mollusk_svm_programs_token;
use solana_axelar_its::state::UserRoles;
use solana_sdk::{account::Account, instruction::Instruction, pubkey::Pubkey};

use crate::new_empty_account;

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
            system_program: solana_sdk_ids::system_program::ID,
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

pub struct TransferInterchainTokenMintershipContext {
    pub mollusk: Mollusk,
    pub payer: (Pubkey, Account),
    pub its_root_pda: (Pubkey, Account),
    pub sender_user_account: (Pubkey, Account),
    pub sender_roles_account: (Pubkey, Account),
    pub token_manager_account: (Pubkey, Account),
    pub destination_user_account: (Pubkey, Account),
    pub destination_roles_account: (Pubkey, Account),
}

impl TransferInterchainTokenMintershipContext {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        mollusk: Mollusk,
        payer: (Pubkey, Account),
        its_root_pda: (Pubkey, Account),
        sender_user_account: (Pubkey, Account),
        sender_roles_account: (Pubkey, Account),
        token_manager_account: (Pubkey, Account),
        destination_user_account: (Pubkey, Account),
    ) -> Self {
        let (destination_roles_pda, _) =
            UserRoles::find_pda(&token_manager_account.0, &destination_user_account.0);

        Self {
            mollusk,
            payer,
            its_root_pda,
            sender_user_account,
            sender_roles_account,
            token_manager_account,
            destination_user_account,
            destination_roles_account: (destination_roles_pda, new_empty_account()),
        }
    }

    pub fn with_custom_destination_roles_account(
        mut self,
        destination_roles_account: Account,
    ) -> Self {
        self.destination_roles_account.1 = destination_roles_account;
        self
    }
}

pub fn transfer_interchain_token_mintership_helper(
    ctx: TransferInterchainTokenMintershipContext,
    checks: Vec<Check>,
) -> (InstructionResult, Mollusk) {
    let program_id = solana_axelar_its::id();

    let ix = Instruction {
        program_id,
        accounts: solana_axelar_its::accounts::TransferInterchainTokenMintership {
            its_root_pda: ctx.its_root_pda.0,
            system_program: solana_sdk_ids::system_program::ID,
            payer: ctx.payer.0,
            sender_user_account: ctx.sender_user_account.0,
            sender_roles_account: ctx.sender_roles_account.0,
            token_manager_account: ctx.token_manager_account.0,
            destination_user_account: ctx.destination_user_account.0,
            destination_roles_account: ctx.destination_roles_account.0,
        }
        .to_account_metas(None),
        data: solana_axelar_its::instruction::TransferInterchainTokenMintership {}.data(),
    };

    let accounts = vec![
        ctx.its_root_pda,
        keyed_account_for_system_program(),
        ctx.payer,
        ctx.sender_user_account,
        ctx.sender_roles_account,
        ctx.token_manager_account,
        ctx.destination_user_account,
        ctx.destination_roles_account,
    ];

    let result = ctx
        .mollusk
        .process_and_validate_instruction(&ix, &accounts, &checks);
    (result, ctx.mollusk)
}
