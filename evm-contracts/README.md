# EVM Helpers

Solidity helpers for constructing Axelar Solana executable payloads.

The gateway payload is an envelope around application-specific bytes:

```text
[encoding scheme byte] [encoded execute payload and Solana account metadata]
```

The `executePayload` bytes are intentionally supplied by the caller. For simple
programs they may be raw bytes. For programs that expect Borsh instruction data,
use `BorshEncoding.sol` to build the inner payload.

These helpers only produce protocol-encoded bytes. Callers are responsible for
keeping the final payload and account list small enough for the intended Solana
transaction, relayer, and destination-program execution path.

## Libraries

- `src/SolanaGatewayPayload.sol`: encodes gateway executable payloads with either
  ABI encoding or Borsh encoding.
- `src/BorshEncoding.sol`: minimal Borsh primitive helpers for caller-owned
  payload construction.

## Encoding Schemes

Borsh gateway payload:

```text
0x00
u32_le executePayload.length
executePayload bytes
u32_le accounts.length
account[0]
account[1]
...
```

Each account is:

```text
bytes32 pubkey
uint8 flags
```

Flags:

```text
bit 0 = isSigner
bit 1 = isWritable
```

ABI gateway payload:

```text
0x01 || abi.encode(bytes executePayload, SolanaAccountRepr[] accounts)
```

## Use With ITS

When sending an interchain token transfer to a Solana program through EVM
`InterchainTokenService.callContractWithInterchainToken`, pass the encoded
gateway payload as the ITS `data` argument.

```solidity
// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.20;

import {
    SolanaAccountRepr,
    SolanaGatewayPayload,
    SolanaGatewayPayloadLib
} from "./src/SolanaGatewayPayload.sol";

interface IInterchainTokenService {
    function callContractWithInterchainToken(
        bytes32 tokenId,
        string calldata destinationChain,
        bytes calldata destinationAddress,
        uint256 amount,
        bytes memory data
    ) external payable;
}

contract SendTokenToSolanaMemo {
    using SolanaGatewayPayloadLib for SolanaGatewayPayload;

    IInterchainTokenService public immutable its;

    constructor(address its_) {
        its = IInterchainTokenService(its_);
    }

    function send(
        bytes32 tokenId,
        bytes32 memoProgramId,
        bytes32 memoCounterPda,
        uint256 amount,
        string calldata memo
    ) external payable {
        SolanaAccountRepr[] memory accounts = new SolanaAccountRepr[](1);
        accounts[0] = SolanaGatewayPayloadLib.writable(memoCounterPda);

        bytes memory data = SolanaGatewayPayload({
            // The memo program reads raw UTF-8 bytes, not a Borsh string.
            executePayload: bytes(memo),
            accounts: accounts
        }).encodeBorsh();

        its.callContractWithInterchainToken{ value: msg.value }(
            tokenId,
            "solana",
            abi.encodePacked(memoProgramId),
            amount,
            data
        );
    }
}
```

For a destination program whose inner instruction data expects Borsh:

```solidity
import { BorshEncoding } from "./src/BorshEncoding.sol";
import {
    SolanaAccountRepr,
    SolanaGatewayPayload,
    SolanaGatewayPayloadLib
} from "./src/SolanaGatewayPayload.sol";

using SolanaGatewayPayloadLib for SolanaGatewayPayload;

bytes memory executePayload = bytes.concat(
    BorshEncoding.encodeU64(orderId),
    BorshEncoding.encodeString(note)
);

SolanaAccountRepr[] memory accounts = new SolanaAccountRepr[](1);
accounts[0] = SolanaGatewayPayloadLib.writable(userStatePda);

bytes memory data = SolanaGatewayPayload({
    executePayload: executePayload,
    accounts: accounts
}).encodeBorsh();
```

For the older ITS metadata API, wrap the same `data` with metadata version `0`:

```solidity
bytes memory metadata = bytes.concat(bytes4(0), data);
```

Then pass `metadata` to `interchainTransfer(..., metadata, gasValue)`.

## Tests

Run Solidity tests:

```sh
forge test
```

From the repository root, run the Rust compatibility fixture:

```sh
cargo test -p solana-axelar-gateway decodes_solidity_borsh_payload_fixture
```

The gateway Rust tests also include a generated compatibility check that runs
Forge when available, emits the Solidity-encoded payload, and decodes that value
in Rust. If Forge is not installed, the test prints a skip warning and returns
success.

```sh
cargo test -p solana-axelar-gateway decodes_forge_generated_borsh_payload
```
