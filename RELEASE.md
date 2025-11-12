# `solana-axelar-*` Crate Release Process

## Prerequisites

### Access

- Access to the [axelar-amplifier-solana](https://github.com/axelarnetwork/axelar-amplifier-solana) repository.
  - Responsible for maintaining the Axelar Amplifier Solana programs and helper libraries.
- Access to the [axelar-contract-deployments](https://github.com/axelarnetwork/axelar-contract-deployments) repository.
  - Responsible for deploying, upgrading, and migrating Solana contracts.

### Changes

- Changes to the `axelar-amplifier-solana` programs and helpers are merged to the `main` branch.
  - All changes should follow conventional commit format to enable proper changelog generation.

## Release Process

### 1. Build and upload pre-release

1. Navigate to the [Build and upload pre-release](https://github.com/axelarnetwork/axelar-amplifier-solana/actions/workflows/pre-release.yaml) GitHub Action.
2. Run the workflow with the following:
    a. Use workflow from: `main` branch
3. If successful, continue; otherwise, address the failure and repeat step 1.

### 2. Dry-Run Release

1. Navigate to the [Dry-Run Release](https://github.com/axelarnetwork/axelar-amplifier-solana/actions/workflows/release-dry-run.yaml) GitHub Action.
2. Run the workflow with the following:
    a. Use workflow from: `main` branch
3. If successful, continue; otherwise, address the failure and repeat step 1.

### 3. Create Release PR

1. Navigate to the [Create Release PR](https://github.com/axelarnetwork/axelar-amplifier-solana/actions/workflows/create-release-pr.yaml) GitHub Action.
2. Run the workflow with the following:
    a. Use workflow from: `main` branch
3. Visit the generated PR and self-review the changes.
4. Reach out to maintainers for approval, and merge the PR.

### 4. Release

1. Navigate to the [Release](https://github.com/axelarnetwork/axelar-amplifier-solana/actions/workflows/release.yaml) GitHub Action.
2. Run the workflow with the following:
    a. Use workflow from: `main` branch

### 5. Verify Release

1. Navigate to the newly released crates on [crates.io](https://crates.io/search?q=solana-axelar).
2. Verify the version numbers for each of the following crates are as expected:
    - `programs/`
        - [`solana-axelar-gateway`](https://crates.io/crates/solana-axelar-gateway)
        - [`solana-axelar-governance`](https://crates.io/crates/solana-axelar-governance)
        - [`solana-axelar-its`](https://crates.io/crates/solana-axelar-its)
        - [`solana-axelar-gas-service`](https://crates.io/crates/solana-axelar-gas-service)
        - [`solana-axelar-multicall`](https://crates.io/crates/solana-axelar-multicall)
3. Verify the GitHub Releases page is as expected: <https://github.com/axelarnetwork/axelar-amplifier-solana/releases>
4. Verify the GitHub Tags page is as expected: <https://github.com/axelarnetwork/axelar-amplifier-solana/tags>
