# Mollusk Harness for Solana Axelar

`mollusk-harness` provides context-backed test harnesses for the Axelar Solana
programs. The goal is to make tests read like end-to-end protocol flows while
keeping the setup for program loading, operator accounts, gateway verifier state,
and token accounts in one place.

The harness is currently an internal workspace crate. It is structured so it can
be published later for downstream programs that integrate with Axelar protocols.

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

## Basic Pattern

Use `new()` for a ready-to-use harness with the protocol initialized. Use
`default()` when testing initialization itself or custom failure setup.

```rust
use mollusk_harness::{GasServiceSetup, GasServiceTestHarness, TestHarness};
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

If the harness is published, this should become configurable so downstream
projects can point at their own artifact directory without relying on workspace
relative paths.

## Publishing TODOs

- Make the deploy artifact directory configurable.
- Decide which program-specific helpers should stay public versus test-only.
- Remove internal-only dependencies before publishing, or gate them behind
  feature flags.
- Add focused examples for gateway-only and ITS end-to-end tests.
- Migrate governance last, then remove the legacy fixture crates.
