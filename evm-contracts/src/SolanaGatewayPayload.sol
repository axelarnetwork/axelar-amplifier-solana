// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.20;

using SolanaGatewayPayloadLib for SolanaGatewayPayload global;

/// @notice Represents a payload that can be executed on Solana via the Axelar gateway.
/// @dev `executePayload` is destination-program-specific. This library only wraps it
///      with the gateway encoding scheme and Solana account metadata. Encoded bytes
///      can still exceed Solana transaction or route-specific message limits; callers
///      are responsible for keeping payload and account sizes executable.
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
    // Must stay in sync with programs/solana-axelar-gateway/src/payload/encoding.rs.
    uint8 internal constant BORSH_SCHEME = 0;
    uint8 internal constant ABI_SCHEME = 1;

    error InvalidEncoding();
    error PayloadLengthOverflow();
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
        // Rust decodes ABI payloads as tuple params, matching `abi.encode(bytes, accounts)`.
        // Do not replace with `abi.encode(payload)`, which encodes a single struct param.
        return abi.encodePacked(bytes1(ABI_SCHEME), abi.encode(payload.executePayload, payload.accounts));
    }

    function encodeBorsh(SolanaGatewayPayload memory payload) internal pure returns (bytes memory encoded) {
        if (payload.executePayload.length > type(uint32).max) revert PayloadLengthOverflow();
        if (payload.accounts.length > type(uint32).max) revert AccountLengthOverflow();

        // TODO: Optimize gas further by copying payload bytes and account pubkeys in 32-byte
        // chunks. This avoids O(n^2) buffer growth, but still writes byte-by-byte.
        encoded = new bytes(1 + 4 + payload.executePayload.length + 4 + payload.accounts.length * 33);

        uint256 offset;
        encoded[offset++] = bytes1(BORSH_SCHEME);

        offset = writeU32(encoded, offset, uint32(payload.executePayload.length));
        for (uint256 i; i < payload.executePayload.length; ++i) {
            encoded[offset++] = payload.executePayload[i];
        }

        offset = writeU32(encoded, offset, uint32(payload.accounts.length));
        for (uint256 i; i < payload.accounts.length; ++i) {
            offset = writeAccountBorsh(encoded, offset, payload.accounts[i]);
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

    function writeU32(bytes memory buffer, uint256 offset, uint32 value) private pure returns (uint256) {
        buffer[offset++] = bytes1(uint8(value));
        buffer[offset++] = bytes1(uint8(value >> 8));
        buffer[offset++] = bytes1(uint8(value >> 16));
        buffer[offset++] = bytes1(uint8(value >> 24));
        return offset;
    }

    function writeAccountBorsh(bytes memory buffer, uint256 offset, SolanaAccountRepr memory accountRepr)
        private
        pure
        returns (uint256)
    {
        for (uint256 i; i < 32; ++i) {
            buffer[offset++] = accountRepr.pubkey[i];
        }
        buffer[offset++] = accountFlags(accountRepr);
        return offset;
    }
}
