// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.20;

import {BorshEncoding} from "./BorshEncoding.sol";

using SolanaGatewayPayloadLib for SolanaGatewayPayload global;

/// @notice Represents a payload that can be executed on Solana via the Axelar gateway.
/// @dev `executePayload` is destination-program-specific. This library only wraps it
///      with the gateway encoding scheme and Solana account metadata.
struct SolanaGatewayPayload {
    bytes executePayload;
    SolanaAccountRepr[] accounts;
}

/// @notice Representation of a Solana account meta used by executable payloads.
struct SolanaAccountRepr {
    bytes32 pubkey;
    bool isSigner;
    bool isWritable;
}

enum SolanaGatewayEncoding {
    Borsh,
    Abi
}

library SolanaGatewayPayloadLib {
    uint8 internal constant BORSH_SCHEME = 0;
    uint8 internal constant ABI_SCHEME = 1;

    error InvalidEncoding();
    error AccountLengthOverflow();

    function encode(SolanaGatewayPayload memory payload, SolanaGatewayEncoding encoding)
        internal
        pure
        returns (bytes memory)
    {
        if (encoding == SolanaGatewayEncoding.Borsh) {
            return encodeBorsh(payload);
        }
        if (encoding == SolanaGatewayEncoding.Abi) {
            return encodeAbi(payload);
        }
        revert InvalidEncoding();
    }

    function encodeAbi(SolanaGatewayPayload memory payload) internal pure returns (bytes memory) {
        return abi.encodePacked(bytes1(ABI_SCHEME), abi.encode(payload.executePayload, payload.accounts));
    }

    function encodeBorsh(SolanaGatewayPayload memory payload) internal pure returns (bytes memory encoded) {
        if (payload.accounts.length > type(uint32).max) revert AccountLengthOverflow();

        encoded = bytes.concat(
            bytes1(BORSH_SCHEME),
            BorshEncoding.encodeBytes(payload.executePayload),
            BorshEncoding.encodeU32(uint32(payload.accounts.length))
        );

        for (uint256 i; i < payload.accounts.length; ++i) {
            encoded = bytes.concat(encoded, encodeAccountBorsh(payload.accounts[i]));
        }
    }

    function payloadHash(bytes memory encodedPayload) internal pure returns (bytes32) {
        return keccak256(encodedPayload);
    }

    function account(bytes32 pubkey, bool isSigner, bool isWritable) internal pure returns (SolanaAccountRepr memory) {
        return SolanaAccountRepr({pubkey: pubkey, isSigner: isSigner, isWritable: isWritable});
    }

    function readonly(bytes32 pubkey) internal pure returns (SolanaAccountRepr memory) {
        return account(pubkey, false, false);
    }

    function writable(bytes32 pubkey) internal pure returns (SolanaAccountRepr memory) {
        return account(pubkey, false, true);
    }

    function signer(bytes32 pubkey, bool isWritable) internal pure returns (SolanaAccountRepr memory) {
        return account(pubkey, true, isWritable);
    }

    function encodeAccountBorsh(SolanaAccountRepr memory accountRepr) internal pure returns (bytes memory) {
        return abi.encodePacked(accountRepr.pubkey, accountFlags(accountRepr));
    }

    function accountFlags(SolanaAccountRepr memory accountRepr) internal pure returns (bytes1) {
        uint8 flags = accountRepr.isSigner ? 1 : 0;
        if (accountRepr.isWritable) {
            flags |= 2;
        }
        return bytes1(flags);
    }
}
