# Solana Axelar Mollusk Harness

`solana-axelar-mollusk-harness` provides context-backed test harnesses for the Axelar Solana
programs. The goal is to make tests read like end-to-end protocol flows while
keeping the setup for program loading, operator accounts, gateway verifier state,
and token accounts in one place.

The crate is intended for Axelar programs and downstream Solana programs that
integrate with Axelar protocols and need end-to-end tests on Mollusk.

## Available Harnesses

- `OperatorsTestHarness` initializes the operators registry and exposes helpers
  for adding, removing, and transferring operators.
- `GasServiceTestHarness` initializes operators plus the gas service treasury and
  exposes instruction builders for gas payments, refunds, and fee collection.
- `GatewayTestHarness` initializes operators, gas service, and gateway verifier
  state, then exposes helpers for message approval, signature verification,
  signer rotation, and contract calls.
- `ItsTestHarness` initializes the full ITS dependency stack, including gateway,
  token programs, and token metadata support.

All harnesses use `MolluskContext<HashMap<Pubkey, Account>>`, so state persists
across instructions within a test.

The main types are re-exported at the crate root:

```rust
use solana_axelar_mollusk_harness::{
    GasServiceSetup, GasServiceTestHarness, GatewaySetup, GatewayTestHarness, ItsTestHarness,
    OperatorsSetup, OperatorsTestHarness, TestHarness,
};
```

## Basic Pattern

Use `new()` for a ready-to-use harness with the protocol initialized. Use
`default()` when testing initialization itself or custom failure setup.

```rust
use solana_axelar_mollusk_harness::{GasServiceSetup, GasServiceTestHarness, TestHarness};
use mollusk_svm::result::Check;
use solana_sdk::pubkey::Pubkey;

#[test]
fn pays_native_gas() {
    let harness = GasServiceTestHarness::new();

    let payer = Pubkey::new_unique();
    let amount = 300_000_000;
    harness.ensure_account_exists_with_lamports(payer, 1_000_000_000);

    let ix = harness.pay_gas_ix(
        payer,
        "ethereum".to_owned(),
        "0xrecipient".to_owned(),
        [0; 32],
        amount,
        payer,
    );

    harness
        .ctx
        .process_and_validate_instruction(&ix, &[Check::success()]);
}
```

## Writing Tests

Prefer harness helpers for protocol setup and PDA derivation, but keep the test's
assertions local. This keeps the harness ergonomic without hiding the behavior
under test.

Good tests generally follow this shape:

1. Create the narrowest harness that has the required dependencies.
2. Create only the extra accounts needed by the scenario.
3. Build the instruction through a harness helper or program helper.
4. Execute with explicit `Check` expectations.
5. Read state back through `get_account_as`, token helpers, or direct account
   checks.

## Program Artifacts

The harness expects SBF artifacts under `../../target/deploy` relative to each
program test crate. Initialization helpers set `SBF_OUT_DIR` to this default via
`ensure_default_sbf_out_dir()`.

Build the programs before running tests that instantiate `new()` harnesses:

```sh
cargo build-sbf
```

Downstream projects that use different artifact locations can set `SBF_OUT_DIR`
before creating a harness, or load programs directly on the returned Mollusk
context when a test needs custom setup.

## Gateway Messages

`GatewaySetup::approve_incoming_messages` runs the full gateway approval flow and
returns the approved message accounts:

```rust
use solana_axelar_gateway::Message;
use solana_axelar_mollusk_harness::{GatewaySetup, GatewayTestHarness};

let harness = GatewayTestHarness::new();
let messages: Vec<Message> = Vec::new();
let approved = harness.approve_incoming_messages(&messages);
```

Use `ensure_approved_incoming_messages` when the test only needs the gateway
state to exist and does not need the returned account data.

## Common Patterns

### Execute A GMP Transfer Into ITS

Use `ItsTestHarness` when the test needs gateway approval plus ITS execution.
The harness initializes the gateway, gas service, operators, token programs, and
ITS root state.

```rust
use solana_axelar_mollusk_harness::{ItsTestHarness, TestHarness};
use solana_sdk::pubkey::Pubkey;

let harness = ItsTestHarness::new();
let token_id = harness.ensure_test_interchain_token();
let receiver = Pubkey::new_unique();

harness.execute_gmp_transfer(
    token_id,
    "ethereum",
    "0xSourceAddress",
    receiver,
    1_000,
    None,
);

let mint = harness.token_mint_for_id(token_id);
let receiver_ata = harness.get_ata_2022_address(receiver, mint);
let receiver_token_account = harness
    .get_token_account(&receiver_ata)
    .expect("receiver token account should exist");

assert_eq!(receiver_token_account.amount, 1_000);
```

### Test An Outbound ITS Transfer

For outbound transfers, create or register a token, mint funds to the user, then
call one of the interchain transfer helpers. Use
`ensure_outgoing_user_interchain_transfer` for user-originated transfers and
`ensure_outgoing_interchain_transfer` when the caller is another program.

```rust
use anchor_spl::token_2022::spl_token_2022;
use solana_axelar_mollusk_harness::{ItsTestHarness, TestHarness};

let harness = ItsTestHarness::new();
let token_id = harness.ensure_test_interchain_token();
let mint = harness.token_mint_for_id(token_id);
let user = harness.get_new_wallet();
let (user_ata, _) = harness.get_or_create_ata_2022_account(user, user, mint);

harness.ensure_mint_interchain_token(token_id, 10_000, harness.operator, user_ata, spl_token_2022::ID);
harness.ensure_outgoing_user_interchain_transfer(
    token_id,
    2_500,
    spl_token_2022::ID,
    user,
    user,
    "ethereum".to_owned(),
    b"0xDestination".to_vec(),
    0,
);
```

### Link A Canonical Token Through GMP

Canonical-token tests usually start from an existing SPL Token 2022 mint. The
harness can create the mint, attach metadata, register it with ITS, and then run
the inbound GMP link-token path.

```rust
use solana_axelar_its::{encoding, state::token_manager};
use solana_axelar_mollusk_harness::ItsTestHarness;
use solana_sdk::pubkey::Pubkey;

let harness = ItsTestHarness::new();
let mint_authority = Pubkey::new_unique();
let (token_mint, token_id) = harness.ensure_test_registered_canonical_token(mint_authority);

let payload = encoding::LinkToken {
    token_id,
    token_manager_type: token_manager::Type::LockUnlock.into(),
    source_token_address: b"0xToken".to_vec(),
    destination_token_address: token_mint.to_bytes().to_vec(),
    params: None,
};

harness.execute_gmp_link_token(token_id, "ethereum", token_mint, payload, Vec::new());
```
