# Relayer Guide: Address Lookup Tables for ITS

This guide explains how the Axelar relayer should use Address Lookup Tables
(ALTs) when submitting ITS Execute transactions on Solana.

---

## Long-lived Inbound ALT

The relayer maintains a single global ALT with **9 entries** that is reused
for every ITS Execute transaction.

| Index | Account | Derivation hint |
|-------|---------|-----------------|
| 0 | `gateway_root_pda` | `["gateway"]` from gateway program |
| 1 | `gateway_event_authority` | `["__event_authority"]` from gateway program |
| 2 | `axelar_gateway_program` | Program ID (static) |
| 3 | `its_root_pda` | `["its"]` from ITS program |
| 4 | `associated_token_program` | Well-known program ID |
| 5 | `system_program` | Well-known program ID |
| 6 | `event_authority` (ITS) | `["__event_authority"]` from ITS program |
| 7 | `spl_token` | Token program ID |
| 8 | `spl_token_2022` | Token2022 program ID |

Both token programs are included so the ALT works regardless of which token
standard a given token uses. Unused entries cost nothing in a transaction.

### Setup

- Create the ALT once. Rent cost is approximately **0.002 SOL**.
- Use it for **all** ITS Execute transactions -- there is no reason to
  conditionally include it.
- Always build **v0 transactions** with this ALT. Using v0 unconditionally
  is simpler than conditional logic and saves **245 bytes** every time.
- Optionally **freeze** the ALT after creation to make it immutable (the
  entries never change for a given deployment).

---

## When to Use a Short-lived Temp ALT (Level 2)

Most ITS Execute transactions fit within the 1232-byte limit using only the
global inbound ALT. However, `execute_interchain_transfer` **with data** can
overflow when the payload is large or the destination program requires many
accounts.

### Decision process

1. **Decode the inbound GMP payload** to determine if it is an
   `InterchainTransfer` with a non-empty `data` field.
2. **Compute the transaction size** with only the global ALT.
3. **If it fits** in 1232 bytes, submit as-is (Level 1).
4. **If it overflows**, create a temp ALT containing all non-signer accounts
   from the instruction:
   - `incoming_message_pda`
   - `signing_pda`
   - `token_manager_pda`
   - `token_mint`
   - `token_manager_ata`
   - `destination`
   - `destination_token_authority`
   - `destination_ata`
   - `interchain_transfer_execute` (destination program)
   - Any user-specified destination program accounts
5. **Wait 1 slot** (~400ms) for ALT activation.
6. **Submit the v0 transaction** referencing both ALTs (global + temp).
7. **Deactivate + close** the temp ALT after execution to reclaim rent.

---

## Technical Notes

### program_id must be direct

The instruction's `program_id` (the ITS program) **cannot** come from an ALT.
It must appear in the transaction's direct `account_keys` array. The runtime
enforces this.

### Signers must be direct

The fee payer and any other transaction signers **cannot** be resolved from
ALTs. They must be in the direct `account_keys`.

### ALTs are read-only during execution

Multiple transactions can reference the same ALT concurrently with **no
contention**. The ALT account is only written during create, extend,
deactivate, and close operations. Normal transaction execution only reads
from it.

### v0 message format

Use `v0::Message::try_compile()` to build the versioned message. It handles
ALT resolution automatically -- pass the instructions and a slice of
`AddressLookupTableAccount` structs.

### Temp ALT lifecycle

1. **Create + extend** in a single legacy transaction.
2. **Wait 1 slot** before the ALT becomes usable for lookups.
3. **Use** the ALT in the v0 ITS Execute transaction.
4. **Deactivate** immediately after use.
5. **Close** after approximately 512 slots (~3-4 minutes) to reclaim the
   rent deposit.

---

## Decision Flowchart (pseudocode)

```rust
fn submit_its_execute(payload, global_alt):
    ix = build_execute_instruction(payload)
    tx_size = compute_v0_tx_size(ix, [global_alt])
    if tx_size <= 1232:
        submit_v0(ix, [global_alt])
    else:
    		if (not_enough_for_all)
      		throw needs_funds();
        temp_alt = create_and_extend_alt(non_signer_accounts(ix))
        wait_one_slot()
        submit_v0(ix, [global_alt, temp_alt])
        deactivate_alt(temp_alt)
```
