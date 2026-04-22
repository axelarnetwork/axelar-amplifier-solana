# Solana GMP & ITS Protocol Overview

> **DRAFT / WIP** -- this is a starting point for detailed documentation.
> Refer to the source code for authoritative behavior.

---

## Gateway (GMP Layer)

The Gateway program is the core cross-chain messaging layer on Solana.

### Outbound (`call_contract`)

Programs or users send cross-chain messages by calling `call_contract`. The
gateway:

1. Hashes the payload on-chain.
2. Emits the payload hash as a program event.
3. Verifies the caller -- either a direct transaction signer, or a program
   invoking via CPI through a signing PDA.

The **gas service** program is used alongside outbound messages to pay for
cross-chain delivery on the destination chain.

### Inbound (`execute`)

The relayer submits approved messages from other chains. The flow:

1. The relayer calls the destination program's execute instruction.
2. The destination program implements the `executable_accounts!` macro pattern,
   which provides the 5-account `AxelarExecuteAccounts` struct.
3. The destination calls `validate_message()`, which:
   - Reconstructs the full payload from `payload_without_accounts` + the
     instruction's account list.
   - Verifies the reconstructed payload hash against the approved message.
4. For pre-encoded payloads (e.g., ITS HubMessages), `validate_message_raw()`
   skips the reconstruction step and verifies the raw payload directly.

---

## ITS (Interchain Token Service)

ITS is built on top of Gateway GMP. It handles token transfers, deployment,
and linking across chains.

### Outbound

**`interchain_transfer`** -- sends tokens cross-chain:
- Burns or locks tokens locally (depending on the token manager type).
- Wraps the transfer into a HubMessage.
- Sends the HubMessage via the gateway's `call_contract`.

**`deploy_remote_interchain_token` / `deploy_remote_canonical_token`** --
requests deployment of a token on a remote chain.

**`link_token`** -- links a custom token to a remote chain's token, creating
a token manager association.

### Inbound

**`execute`** -- the main GMP entrypoint for ITS. It:
1. Receives an approved HubMessage from the gateway.
2. Decodes the inner ITS message type.
3. Dispatches via CPI to one of the specific handlers below.

**`execute_interchain_transfer`** -- mints or unlocks tokens to the
destination. If the transfer includes a `data` field, it additionally invokes
the destination program via CPI, passing an `AxelarMessagePayload` containing
both the user's payload bytes and the destination program's accounts.

**`execute_deploy_interchain_token`** -- creates a new token mint and
associated token manager on Solana.

**`execute_link_token`** -- registers a custom token manager for a
previously-deployed token.

### Key Concepts

**HubMessage wrapping.** All ITS messages are wrapped in a HubMessage
envelope before passing through the gateway:
- Outbound: `SendToHub { destination_chain, message }`
- Inbound: `ReceiveFromHub { source_chain, message }`

**Token Manager types.** Each token registration uses one of these strategies:
- `NativeInterchainToken` -- ITS-created token, full mint authority
- `MintBurn` -- custom token where ITS has mint/burn authority
- `MintBurnFrom` -- like MintBurn but uses `burn_from` (with allowance)
- `LockUnlock` -- ITS locks tokens in escrow on send, unlocks on receive
- `LockUnlockFee` -- like LockUnlock but accounts for transfer fees

**AxelarMessagePayload.** For `execute_interchain_transfer` with data, the
`data` field is an ABI-encoded `AxelarMessagePayload` containing:
- The user's raw payload bytes
- A list of destination program account metas (pubkeys + is_signer + is_writable)

This lets the destination program receive both the cross-chain data and the
Solana accounts it needs in one CPI call.

**ITS uses `validate_message_raw()`.** Because the ITS payload is
pre-encoded as a HubMessage, ITS calls `validate_message_raw()` (not
`validate_message()`) to verify the hash without account reconstruction.

**Flow tracking.** Token managers track cumulative flow in and out per epoch.
This enables rate limiting -- transfers that would exceed the configured
flow limit for the current epoch are rejected.

### Account Patterns

**`executable_accounts!` macro.** Generates the `AxelarExecuteAccounts`
struct with 5 accounts:
1. `incoming_message_pda` -- the approved message PDA
2. `signing_pda` -- the gateway's signing PDA for CPI verification
3. `gateway_root_pda` -- the gateway's root configuration PDA
4. `event_authority` -- for gateway event emission
5. `gateway_program` -- the gateway program itself

**`#[event_cpi]`** (Anchor). Adds 2 implicit accounts to any instruction
that emits events:
1. `event_authority` -- PDA used to sign the event CPI
2. `program` -- the current program's ID

**Token operations.** ITS interacts with tokens through the SPL Token and
Token2022 (Token Extensions) program interfaces, supporting both standard
and extension tokens.
