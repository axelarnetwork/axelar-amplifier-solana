# Solana Protocol Limits

Solana transactions are hard-capped at **1232 bytes**. This limits the amount of custom data and accounts that can be included in cross-chain messages.

---

## Inbound (External → Solana)

### GMP Execute

When receiving a cross-chain GMP message, the destination program receives a payload and accounts. Accounts are reconstructed on-chain from the transaction's account list for hash verification — they are not duplicated in the instruction data.

**Budget: ~662 bytes** shared between payload and accounts. Each account costs **33 bytes**, leaving the rest for payload data.

### ITS Execute — InterchainTransfer with data

When receiving a cross-chain token transfer with attached data, the destination program receives a payload and accounts via CPI from the ITS program.

Due to the current protocol design, each destination account is encoded in both the instruction data and the transaction's account list. Each account costs **66 bytes**.

**Standard budget: ~374 bytes** shared between payload and accounts.

For transfers requiring more space, the relayer creates an on-demand temporary Address Lookup Table covering all non-signer accounts. This reduces the per-account cost to **34 bytes** and increases the budget to **~619 bytes**.

---

## Outbound (Solana → External)

### GMP call_contract

When sending a cross-chain GMP message from Solana, the payload is passed as instruction data to `call_contract`. This is typically called via CPI from another program — in that case the outer program's accounts and instruction overhead reduce the available space.

**Max payload (standalone, via CPI): ~890 bytes.** Direct wallet callers (no CPI) have a higher limit of ~922 bytes.

### ITS InterchainTransfer

When sending tokens cross-chain via `interchain_transfer`, the optional `data` field carries the user's custom payload.

**Max `data` payload: ~369 bytes** (user wallet) or **~315 bytes** (CPI caller).

For convenience, we offer an Address Lookup Table with 12 static protocol accounts (gateway, gas service, ITS, and both token program IDs). Using this ALT increases the limits to **~707 bytes** (user wallet) or **~653 bytes** (CPI caller).
