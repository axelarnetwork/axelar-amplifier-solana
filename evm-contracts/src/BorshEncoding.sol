// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.20;

/// @notice Minimal Borsh encoding helpers for building Solana instruction payloads.
/// @dev This is intentionally not a full schema system. Callers remain responsible
///      for matching the destination program's exact Borsh layout.
library BorshEncoding {
    error LengthOverflow();

    function encodeU8(uint8 value) internal pure returns (bytes memory) {
        return abi.encodePacked(value);
    }

    function encodeU16(uint16 value) internal pure returns (bytes memory) {
        return abi.encodePacked(bytes1(uint8(value)), bytes1(uint8(value >> 8)));
    }

    function encodeU32(uint32 value) internal pure returns (bytes memory) {
        return abi.encodePacked(
            bytes1(uint8(value)), bytes1(uint8(value >> 8)), bytes1(uint8(value >> 16)), bytes1(uint8(value >> 24))
        );
    }

    function encodeU64(uint64 value) internal pure returns (bytes memory) {
        return abi.encodePacked(
            bytes1(uint8(value)),
            bytes1(uint8(value >> 8)),
            bytes1(uint8(value >> 16)),
            bytes1(uint8(value >> 24)),
            bytes1(uint8(value >> 32)),
            bytes1(uint8(value >> 40)),
            bytes1(uint8(value >> 48)),
            bytes1(uint8(value >> 56))
        );
    }

    function encodeU128(uint128 value) internal pure returns (bytes memory encoded) {
        encoded = new bytes(16);
        for (uint256 i; i < 16; ++i) {
            encoded[i] = bytes1(uint8(value >> (i * 8)));
        }
    }

    function encodeBool(bool value) internal pure returns (bytes memory) {
        return abi.encodePacked(value ? bytes1(uint8(1)) : bytes1(uint8(0)));
    }

    function encodeFixedBytes32(bytes32 value) internal pure returns (bytes memory) {
        return abi.encodePacked(value);
    }

    function encodeBytes(bytes memory value) internal pure returns (bytes memory) {
        return bytes.concat(encodeLength(value.length), value);
    }

    function encodeString(string memory value) internal pure returns (bytes memory) {
        return encodeBytes(bytes(value));
    }

    function encodeSomeBytes(bytes memory value) internal pure returns (bytes memory) {
        return bytes.concat(bytes1(uint8(1)), encodeBytes(value));
    }

    function encodeSomeString(string memory value) internal pure returns (bytes memory) {
        return bytes.concat(bytes1(uint8(1)), encodeString(value));
    }

    function encodeNone() internal pure returns (bytes memory) {
        return abi.encodePacked(bytes1(uint8(0)));
    }

    function appendU8(bytes memory buffer, uint8 value) internal pure returns (bytes memory) {
        return bytes.concat(buffer, encodeU8(value));
    }

    function appendU16(bytes memory buffer, uint16 value) internal pure returns (bytes memory) {
        return bytes.concat(buffer, encodeU16(value));
    }

    function appendU32(bytes memory buffer, uint32 value) internal pure returns (bytes memory) {
        return bytes.concat(buffer, encodeU32(value));
    }

    function appendU64(bytes memory buffer, uint64 value) internal pure returns (bytes memory) {
        return bytes.concat(buffer, encodeU64(value));
    }

    function appendU128(bytes memory buffer, uint128 value) internal pure returns (bytes memory) {
        return bytes.concat(buffer, encodeU128(value));
    }

    function appendBool(bytes memory buffer, bool value) internal pure returns (bytes memory) {
        return bytes.concat(buffer, encodeBool(value));
    }

    function appendBytes(bytes memory buffer, bytes memory value) internal pure returns (bytes memory) {
        return bytes.concat(buffer, encodeBytes(value));
    }

    function appendString(bytes memory buffer, string memory value) internal pure returns (bytes memory) {
        return bytes.concat(buffer, encodeString(value));
    }

    function encodeLength(uint256 length) internal pure returns (bytes memory) {
        if (length > type(uint32).max) revert LengthOverflow();
        return encodeU32(uint32(length));
    }
}
