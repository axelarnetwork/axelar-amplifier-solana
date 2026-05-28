// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.20;

import {BorshEncoding} from "../src/BorshEncoding.sol";

contract BorshEncodingTest {
    function testIntegerEncodingIsLittleEndian() public pure {
        assertBytesEq(BorshEncoding.encodeU8(0x12), hex"12");
        assertBytesEq(BorshEncoding.encodeU16(0x1234), hex"3412");
        assertBytesEq(BorshEncoding.encodeU32(0x12345678), hex"78563412");
        assertBytesEq(BorshEncoding.encodeU64(0x0102030405060708), hex"0807060504030201");
        assertBytesEq(
            BorshEncoding.encodeU128(0x0102030405060708090a0b0c0d0e0f10), hex"100f0e0d0c0b0a090807060504030201"
        );
    }

    function testBoolBytesStringAndOptions() public pure {
        assertBytesEq(BorshEncoding.encodeBool(true), hex"01");
        assertBytesEq(BorshEncoding.encodeBool(false), hex"00");
        assertBytesEq(BorshEncoding.encodeBytes(hex"aabbcc"), hex"03000000aabbcc");
        assertBytesEq(BorshEncoding.encodeString("memo"), hex"040000006d656d6f");
        assertBytesEq(BorshEncoding.encodeOptionBytes(hex"aabb", true), hex"0102000000aabb");
        assertBytesEq(BorshEncoding.encodeOptionBytes(hex"aabb", false), hex"00");
        assertBytesEq(BorshEncoding.encodeOptionString("hi", true), hex"01020000006869");
        assertBytesEq(BorshEncoding.encodeOptionString("hi", false), hex"00");
    }

    function testAppendHelpers() public pure {
        bytes memory encoded = hex"99";
        encoded = BorshEncoding.appendU32(encoded, 2);
        encoded = BorshEncoding.appendString(encoded, "ok");
        encoded = BorshEncoding.appendBool(encoded, true);

        assertBytesEq(encoded, hex"9902000000020000006f6b01");
    }

    function testFuzzU32EncodingMatchesLittleEndian(uint32 value) public pure {
        bytes memory encoded = BorshEncoding.encodeU32(value);

        require(encoded.length == 4, "bad u32 length");
        require(uint8(encoded[0]) == uint8(value), "bad u32 byte 0");
        require(uint8(encoded[1]) == uint8(value >> 8), "bad u32 byte 1");
        require(uint8(encoded[2]) == uint8(value >> 16), "bad u32 byte 2");
        require(uint8(encoded[3]) == uint8(value >> 24), "bad u32 byte 3");
    }

    function testFuzzU64EncodingMatchesLittleEndian(uint64 value) public pure {
        bytes memory encoded = BorshEncoding.encodeU64(value);

        require(encoded.length == 8, "bad u64 length");
        for (uint256 i; i < 8; ++i) {
            require(uint8(encoded[i]) == uint8(value >> (i * 8)), "bad u64 byte");
        }
    }

    function testFuzzU128EncodingMatchesLittleEndian(uint128 value) public pure {
        bytes memory encoded = BorshEncoding.encodeU128(value);

        require(encoded.length == 16, "bad u128 length");
        for (uint256 i; i < 16; ++i) {
            require(uint8(encoded[i]) == uint8(value >> (i * 8)), "bad u128 byte");
        }
    }

    function testFuzzBytesEncoding(bytes memory value) public pure {
        if (value.length > 512) return;

        bytes memory encoded = BorshEncoding.encodeBytes(value);

        require(encoded.length == value.length + 4, "bad bytes length");
        assertBytesEq(slice(encoded, 0, 4), BorshEncoding.encodeU32(uint32(value.length)));
        assertBytesEq(slice(encoded, 4, value.length), value);
    }

    function testFuzzAppendEqualsConcat(bytes memory prefix, uint32 value, string memory text) public pure {
        if (prefix.length > 256 || bytes(text).length > 256) return;

        bytes memory appended = BorshEncoding.appendString(BorshEncoding.appendU32(prefix, value), text);
        bytes memory expected = bytes.concat(prefix, BorshEncoding.encodeU32(value), BorshEncoding.encodeString(text));

        assertBytesEq(appended, expected);
    }

    function slice(bytes memory input, uint256 start, uint256 length) private pure returns (bytes memory output) {
        require(input.length >= start + length, "slice out of bounds");
        output = new bytes(length);
        for (uint256 i; i < length; ++i) {
            output[i] = input[start + i];
        }
    }

    function assertBytesEq(bytes memory actual, bytes memory expected) private pure {
        require(keccak256(actual) == keccak256(expected), "bytes mismatch");
    }
}
