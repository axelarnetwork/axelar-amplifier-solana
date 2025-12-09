
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
Each `program_id` will be assigned a `transaction_pda` which is owned by the executable program and stores the transaction to be executed by the program. The `transaction_pda` should be derived by only a single seed: `keccak256('relayer-discovery-transaction') = 0xa57128349132c58c5700674195df81ef5ee89bc36f0e9676bae7e1479b7fcede`. The contents of this pda should strictly be the Anchor serialised data of the `RelayerTransaction` struct:

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
// This is peseudocode.
while(!relayer_transaction_is_final) {
	relayer_transaction = relayer_trnasaction.simulate()
		.return_data
		.decode()
}
relayer_transaction.execute()
``` 

### An Example: Test Discoverable
Look at [this](../../programs/solana-axelar-test-discoverable/) for a working example.
#### Init
A one time call has to be made to the [`Init`](../../programs/solana-axelar-test-discoverable/src/instructions/init.rs) instruction to setup the `transaction_pda`.

The stored relayer transaction points to the [`GetTransaction`](../../programs/solana-axelar-test-discoverable/src/instructions/get_transaction.rs) instruction which returns the executable function.

This points to the [`Execute`](../../programs/solana-axelar-test-discoverable/src/instructions/execute.rs) instruction which is to be called by the relayer.

#### Relayer
The relayer needs to make a few different RPC calls to a node:
1. Call `getAccountInfo` for the  `transaction_pda` initiated by the `solana_axelar_memo_executable`. [`relayer_discovery::find_transaction_pda`](./src/lib.rs#209) can be used to determine the `transaction_pda`.
2. Parse the response into a `RelayerTransaction` object. [`relayer_instruction::convert_transaction`](./src/lib.rs#170) can be used to convert this into a Discovery instruction pointing to `get_transaction`. Call `simulateTransaction` with this information since the `RelayerTransaction` was `Discovery` instead of `Final`.
4. The result data now can be converted to a `RelayerTransaction::Final`. It requires that a `payer` is passed as a signer and some funds are requested. If the gas covers the amount requested alongside what would be needed for the execution (another `simulateTransaction` is needed to determine this, but it is not instrumental for this example) then make sure the `payer` has exactly the funds requested (to ensure loss of additional funds).
5. Finally execute the `Final` transaction, passing the properly funded `payer`.

### Explanation
The reason why we need an intermediate call ([`GetTransaction`](../../programs/solana-axelar-test-discoverable/src/instructions/get_transaction.rs)) is because depending on the `storage_id` a different pda is required for execution, which is calculated there. Also the executable accounts required depend on the `command_id` which is different for every call.