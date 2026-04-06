# Solana-Axelar Interoperability

This repository contains the integration work between Solana and Axelar, enabling seamless cross-chain communication. The project includes General Message Passing (GMP) contracts and other Axelar core components.

## Table of Contents

- [Repository contents](#repository-contents)
  - [Solana programs](#solana-programs)
  - [Utility crates](#utility-crates)
  - [Related repositories](#related-repositories)
- [Getting Started](#getting-started)
  - [Prerequisites](#prerequisites)
  - [Building](#building)
  - [Testing](#testing)
  - [IDL generation](#idl-generation)

## Repository contents

### Solana programs

- [**Gateway**](programs/solana-axelar-gateway): The core contract responsible for authenticating GMP messages.
- [**Interchain Token Service**](programs/solana-axelar-its): Bridge tokens between chains.
- [**Gas Service**](programs/solana-axelar-gas-service): Used for gas payments for the relayer.
- [**Governance**](programs/solana-axelar-governance): The governing entity over on-chain programs, responsible for program upgrades.
- [**Operators**](programs/solana-axelar-operators): Manages operator roles and permissions.
- [**Memo**](programs/solana-axelar-memo): An example program that sends and receives GMP messages.

### Utility crates

- [**solana-axelar-std**](crates/solana-axelar-std): Primitive types, encoding and hashing utilities shared across programs.

### Related Repositories

- [**Solana Relayer**](https://github.com/axelarnetwork/axelar-relayer-solana): The off-chain entity that will route your messages to and from Solana.
- [**Relayer Core**](https://github.com/commonprefix/axelar-relayer-core): Used as a core building block for the Solana Relayer.
- [**Multisig Prover**](https://github.com/axelarnetwork/axelar-amplifier/tree/main/contracts/multisig-prover): The entity on the Axelar chain that is responsible for encoding the data for the Relayer and the Solana Gateway.
- [**Chain Codec Solana**](https://github.com/axelarnetwork/axelar-amplifier/tree/main/contracts/chain-codec-solana): Used by Multisig Prover for Solana-specific encodings.
- [**Utility Scripts**](https://github.com/axelarnetwork/axelar-contract-deployments): Contract deployment scripts and resources for Axelar.

## Getting Started

### Prerequisites

Install all Solana and Anchor development dependencies. See the [Anchor installation guide](https://www.anchor-lang.com/docs/installation) for details.

On Mac/Linux you can install everything with:

```bash
curl --proto '=https' --tlsv1.2 -sSfL https://solana-install.solana.workers.dev | bash
```

### Building

```bash
# Build all programs (default network: devnet-amplifier)
anchor build

# Build for a specific network
cargo xtask build --network mainnet
```

### Testing

```bash
cargo xtask test
```

### IDL generation

```bash
anchor idl build
```

### Linting

```bash
# Runs clippy + fmt check
cargo xtask check
```
