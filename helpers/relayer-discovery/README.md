
# Relayer Discovery

  

The only information about an incoming contract call through axelar are

  

- Command Id: A unique identifier for the command approving the call, 32 bytes long.

- Source Chain: The name of the source chain, as a String

- Source Address: The caller, as a String.

- Destination Chain: “Sui” for the purposes of this document

- Destination Address: The destination, a Solana address.

  

The destination address will be the program id of a program. However there is no way for a relayer to know what they are supposed to call to get the call to be executed, since they don't know the list of accounts that need to be passed in.

  

## Relayer Discovery
Relayer discovery does not need to be a specific program on Solana. This is because programs can create accounts with predetermined addresses that 
Each `program_id` will be assigned a `transaction_pda` which is owned by the executable program and stores the transaction to be executed by the program. The `transaction_pda` should be derived by only a single seed: `keccak256('relayer-discovery-transaction') = 0xa57128349132c58c5700674195df81ef5ee89bc36f0e9676bae7e1479b7fcede`. This contents of this pda should strictly be the Borsh serialised data of the `RelayerTransaction` struct:

```rust
/// A single piece of data to be passed by the relayer. Each of these can be converted to Vec<u8>.
pub enum RelayerData {
	/// Some raw bytes.
	Bytes(Vec<u8>),
	/// The message.
	Message,
	/// The payload, length prefixed.
	Payload,
	/// The payload, length omitted.
	PayloadRaw,
	/// The command id. Can also be abtained by using the `Message`, but it is added as an option for convenience.
	CommandId,
}

/// This can be used to specify an account that the relayer will pass to the executable. This can be converted to an `AccountMeta` by the relayer.
pub enum RelayerAccount {
	/// This variant specifies a specific account. This account cannot be a signer (see `Payer` below).
	Account{
		/// The pubkey of the account.
		pubkey: Pubkey,
		/// Whether or not this account is writable.
		is_writable: bool,
	},
	/// An account that has the payload as its data. This account if and only if it is requested by the executable. This should only be specified once per instruction.
	MessagePayload,
	/// A signer account that has the amount of lamports specified. These lamports will be subtracted from the gas for the execution of the program. 
	/// This can be specified multiple times per instruction, and multiple payer accounts, funded differently will be provided. (Do we want this?)
	Payer(u64)
}

/// A relayer instruction, that the relayer can convert to an `Instruction`.
pub struct RelayerInstruction {
	/// The program_id. Note that this means that an executable can request the entrypoint be a different program (which would have to call the executable to validate the message).
	pub program_id: Pubkey,
	/// The instruction accounts. These need to be ordered properly.
	pub accounts: Vec<RelayerAccount>,
	/// The instruction data. These will be concatenated.
	pub data: Vec<RelayerData>,
}


/// A relayer transaction, that the relayer can convert to regular transaction.
pub enum RelayerTransaction {
	/// This series of instructions should be executed.
	Final(Vec<RelayerInstruction>),
	/// This instruction should be simulated to eventually get a `Final` transaction.
	Discovery(RelayerInstruction),
}
```

Each `RelayerInstruction` can be converted into a regular `Instruction` by doing the following conversions:

- Each entry of `accounts` is either a hardcoded account, the `system_account`, the `incoming_message_pda`, the `message_payload_pda` or a `payer` account, with the specified `lamports`. These lamports are subtracted from the gas offered for the transaction. This last account is essential for executable to be able to write onto memory.
- `data` is converted to `Vec<u8>` by concatenating each of its elements together, with `Bytes` just being vectors, and `Message` being the Borsh serialized version of the [`Message`](https://github.com/eigerco/axelar-amplifier-solana/blob/next/solana/crates/axelar-solana-encoding/src/types/messages.rs#L53) to be executed. 

To figure out what to call the relayer needs to [obtain](https://solana.com/docs/rpc/http/getaccountinfo) this data and then run the following logic, [simulating](https://solana.com/docs/rpc/http/simulatetransaction) transactions when needed:
```
while(!relayer_transaction_is_final) {
	relayer_transaction = relayer_trnasaction.simulate().return_data.decode()
}
relayer_transaction.execute()
``` 

### An Example: Memo Discoverable
Look at [this](../../programs/solana-axelar-memo-discoverable/) for a working example.
#### Init
A one time call has to be made to the `Init` instruction to setup the `transaction_pda`.
```rust
// The relayer transaction to be stored. This should point to the `GetTransaction` entrypoint.
RelayerTransaction::Discovery(RelayerInstruction {
	// We want the relayer to call this program.
	program_id: crate::ID,
	// No accounts are required for this.
	accounts: vec![
	],
	// The data we need to find the final transaction.
	data: vec![
		// We can easily get the discriminaator thankfully. Note that we need `instruction::GetTransaction` and not `instructions::GetTransaction`.
		RelayerData::Bytes(Vec::from(GetTransaction::DISCRIMINATOR)),
		// We do not want to prefix the payload with the length as it is decoded into a struct as opposed to a `Vec<u8>`.
		RelayerData::PayloadRaw,
		// The command id, which is the only thing required (alongside this crate's id) to derive all the accounts required by the gateway.
		RelayerData::CommandId,
	],
}).init(
	&crate::id(),
	&ctx.accounts.system_program.to_account_info(),
	&ctx.accounts.payer.to_account_info(),
	&ctx.accounts.relayer_transaction,
)?;
Ok(())
```
The stored relayer transaction points to the `GetTransaction` instruction which returns the executable function.
```rust
let counter_pda = Counter::get_pda(payload.storage_id).0;
Ok(RelayerTransaction::Final(
	// A single instruction is required. Note that we could be fancy and check whether the counter_pda is initialized (which would required one more discovery transaction be performed),
	// And only if it is not initialized prepend a transaction that initializes it. Then we could omit the `payer` and `system_program` accounts from the actual execute instruction.
	vec![
	RelayerInstruction {
		// We want this program to be the entrypoint.
		program_id: crate::id(),
		// The accounts needed.
		accounts: [
		// First we need the executable accounts.
		relayer_discovery::executable_relayer_accounts(&command_id, &crate::id()), 
		// Followed by the accounts needed to modify storage of the executable.
		vec![
			RelayerAccount::Payer(1000000000),
			RelayerAccount::Account { pubkey: counter_pda, is_writable: true },
			RelayerAccount::Account { pubkey: system_program::ID, is_writable: false },
		]
		].concat(),
		// The data needed.
		data: vec! [
			// We can easily get the discriminaator thankfully. Note that we need `instruction::Execute` and not `instructions::Execute`.
			RelayerData::Bytes(Vec::from(Execute::DISCRIMINATOR)),
			// We do not want to prefix the payload with the length as it is decoded into a struct as opposed to a `Vec<u8>`.
			RelayerData::PayloadRaw,
			// The message, which is needed for the gateway.
			RelayerData::Message,
		],
	}  
	]
))
```
This points to the `Execute` instruction which is to be called by the relayer.
```rust
#[derive(Accounts)]
#[instruction(payload: Payload)]
/// The execute entrypoint.
pub struct Execute<'info> {
    // GMP Accounts
    pub executable: AxelarExecuteAccounts<'info>,

    #[account(mut)]
    pub payer: Signer<'info>,

    // The counter account
    #[account(
        init_if_needed,
        space = Counter::DISCRIMINATOR.len() + Counter::INIT_SPACE,
        payer = payer,
        seeds = [Counter::SEED_PREFIX, &payload.storage_id.to_le_bytes()], 
        bump
    )]
    pub counter: Account<'info, Counter>,

    pub system_program: Program<'info, System>,
}

/// This function keeps track of how many times a message has been received for a given `payload.storage_id`, and logs the `payload.memo`.
pub fn execute_handler(
    ctx: Context<Execute>,
    payload: Payload,
    message: Message,
) -> Result<()> {
	...
}
```
#### Relayer
The relayer needs to make a few different calls to an node:
1. Call `getAccountInfo` for the  `transaction_pda` initiated by the `solana_axelar_memo_executable` 
2. Parse the response into a `RelayerTransaction` object. Convert this into a regular instruction (`get_transaction`). Call `simulateTransaction` with this information since the `RelayerTransaction` was `Discovery` instead of `Final`.
4. The result data now can be converted to a `RelayerTransaction::Final`. It requires that a `payer` is passed as a signer and some funds are requested. If the gas covers the amount requested alongside what would be needed for the execution (another `simulateTransaction` is needed to determine this, but it is not instrumental for this example) then make sure the `payer` has exactly the funds requested (to ensure loss of additional funds).
5. Finally execute the `Final` transaction, passing the properly funded `payer`.

### Explanation
The reason why we need an intermediate call (`GetTransaction`) is because depending on the `storage_id` a different pda is required for execution, which is calculated there. Also the executable accounts required depend on the `command_id` which is different for every call.