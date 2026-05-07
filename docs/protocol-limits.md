# Solana Protocol Limits

Solana hard-caps a transaction at **1232 bytes** (the entire packet —
signatures + message). All numbers in this document are produced by the
verification tests in this repo, which compile real `v0::Message`s and
walk the wire format byte-for-byte:

- `programs/solana-axelar-its/tests/test_transaction_sizes.rs::production_realistic_sizes`
- `programs/solana-axelar-its/tests/test_transaction_sizes_outbound.rs::outbound_realistic_sizes`
- `programs/solana-axelar-gateway/tests/test_transaction_sizes.rs::call_contract_realistic_sizes`

The numbers below assume the typical wire format that Solana clients (and
the relayer) actually send:

| | |
|---|---|
| Signatures | 1 (the payer) |
| Compute-budget instructions | 2 (`set_compute_unit_price` + `set_compute_unit_limit`) — adds ~52 B |
| Message version | v0 (legacy when no ALT) |
| Address Lookup Tables | per the relayer's `includer.rs` strategy (see below) |

If you build outbound transactions yourself and *omit* the compute-budget
instructions, add ~52 B back to every "max payload" figure below.

---

## Inbound (External → Solana)

The relayer (`axelar-relayer-solana`, PR #42) follows a tiered strategy:

1. **No ALT** for non-ITS GMP destinations (custom `executable` programs and
   governance).
2. **Global ITS ALT** — always attached to ITS Execute. Contains 9 static
   accounts (gateway PDAs + program, ITS PDAs + program, both token
   programs, system + ATA programs).
3. **Ephemeral ALT** — created on-demand when an ITS InterchainTransfer
   *with data* still overflows after applying the global ALT. Covers every
   non-signer, non-`program_id` account that isn't already in the global
   ALT (per-message PDAs, per-token PDAs, destination accounts, user
   accounts). The relayer creates it, uses it, then deactivates and closes
   it. The lifecycle cost is paid out of the message's
   `available_gas_balance`.

### GMP Execute (non-ITS destination)

Accounts are reconstructed on-chain from the transaction's account list
for hash verification — they are *not* duplicated in the instruction data.
Per-account cost: **33 B** (32 pubkey + 1 instruction-account index).

| Program/payload accounts | Max payload (bytes) |
|---:|---:|
| 0 | 608 |
| 1 | 575 |
| 2 | 542 |
| 3 | 510 |
| 5 | 444 |
| 8 | 345 |
| 10 | 279 |
| 15 | 114 |
| 20 | 0 (overflow) |

### ITS Execute → DeployInterchainToken / LinkToken

Fixed-size; both fit comfortably with the global ALT and have no
user-supplied payload:

| Instruction | Tx size | Headroom |
|---|---:|---:|
| DeployInterchainToken | 948 B | 284 B |
| LinkToken | 860 B | 372 B |

### ITS Execute → InterchainTransfer (no data)

Fits with **367 B headroom** (865 / 1232 B used).

### ITS Execute → InterchainTransfer (with data)

User-supplied accounts appear in *both* the encoded
`AxelarMessagePayload` (data) *and* the transaction's
`remaining_accounts`, so they cost more than other inbound accounts.

#### Level 1 — global ITS ALT only

Per-user-account cost: **66 B** (33 in payload data + 33 in tx).

| User accounts | Max data payload |
|---:|---:|
| 0 | 289 B |
| 1 | 223 B |
| 2 | 157 B |
| 3 | 91 B |
| 4 | 25 B |
| 5+ | overflow → relayer creates ephemeral ALT |

#### Level 2 — global + ephemeral ALT

The relayer creates the ephemeral ALT automatically when Level 1
overflows. Per-user-account cost drops to **35 B** (33 in payload data +
1 ix-account index + 1 ALT-readonly index).

| User accounts | Max data payload |
|---:|---:|
| 0 | 534 B |
| 1 | 499 B |
| 2 | 464 B |
| 3 | 429 B |
| 5 | 359 B |
| 8 | 254 B |
| 10 | 184 B |
| 15 | 9 B |

> The ephemeral ALT lifecycle (create + deactivate + close) costs 3
> additional transactions paid by the message's gas balance. For
> InterchainTransfer with no data, no payload accounts, or small payloads
> that fit at Level 1, no ephemeral ALT is created.

---

## Outbound (Solana → External)

Outbound transactions are built by the user (or by a CPI caller). The
limits depend on whether you use an Address Lookup Table.

### gateway.call_contract (direct signer)

Max payload: **868 B** (with compute-budget instructions; 922 B without).

When `call_contract` is called via CPI from another program it does *not*
appear as a top-level transaction instruction — it's embedded in the
outer instruction. The payload budget then comes from whatever space the
outer transaction has left.

A 2-account ALT covering `gateway_root_pda` + `event_authority` saves
only ~28 B and is generally not worth the complexity.

### ITS.interchain_transfer

| Caller | No ALT | Shared ALT (11 entries) |
|---|---:|---:|
| User wallet (direct signer) | **315 B** | **622 B** |
| CPI caller (program_id + 2 seeds) | **261 B** | **568 B** |

A protocol-managed shared ALT for outbound is **not** deployed
automatically today — users wanting the larger budget should construct
one themselves and pass it on every `interchain_transfer` send.

The recommended shared-ALT contents are:

- Gateway: `gateway_root_pda`, `gateway_event_authority`,
  `gateway_program`, `call_contract_signing_pda` (ITS-derived)
- Gas service: `gas_treasury` *(writable)*, `gas_service`,
  `gas_event_authority`
- ITS: `its_root_pda`, `event_authority`
- Common: `system_program`, `token_program`

A 12th entry can hold the *other* token program (so the ALT works for
both SPL Token and Token-2022 mints), but only one of the two appears in
any given tx — v0 compile won't reference it, so it adds no per-tx
benefit beyond the 11.

CPI callers add the caller's `program_id` and one PDA per
`caller_pda_seeds` entry to the static account list, costing ~32 B
each — that's why the CPI rows are ~54 B smaller than the direct-signer
rows.

---

## Reproducing these numbers

```sh
cargo test -p solana-axelar-its     --test test_transaction_sizes          production_realistic_sizes -- --nocapture
cargo test -p solana-axelar-its     --test test_transaction_sizes_outbound outbound_realistic_sizes   -- --nocapture
cargo test -p solana-axelar-gateway --test test_transaction_sizes          call_contract_realistic_sizes -- --nocapture
```

Each test compiles a real `solana_sdk::message::v0::Message` (the same
type the relayer's `estimate_v0_tx_size` produces) and walks the wire
format. Update those tests if the instruction surface changes; they are
the source of truth for this document.
