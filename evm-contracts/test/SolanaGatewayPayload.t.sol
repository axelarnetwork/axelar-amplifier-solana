// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.20;

import {
    SolanaAccountRepr,
    SolanaGatewayEncoding,
    SolanaGatewayPayload,
    SolanaGatewayPayloadLib
} from "../src/SolanaGatewayPayload.sol";

contract SolanaGatewayPayloadTest {
    using SolanaGatewayPayloadLib for SolanaGatewayPayload;

    event EncodedBorshPayload(bytes payload);

    function testBorshEncodingEmptyPayloadAndAccounts() public pure {
        SolanaAccountRepr[] memory accounts = new SolanaAccountRepr[](0);
        bytes memory encoded = SolanaGatewayPayload({executePayload: "", accounts: accounts}).encodeBorsh();

        assertBytesEq(encoded, hex"000000000000000000");
    }

    function testBorshEncodingPayloadAndAccountFlags() public pure {
        SolanaAccountRepr[] memory accounts = new SolanaAccountRepr[](4);
        accounts[0] = SolanaGatewayPayloadLib.account(bytes32(uint256(0x1111)), true, true);
        accounts[1] = SolanaGatewayPayloadLib.signer(bytes32(uint256(0x2222)), false);
        accounts[2] = SolanaGatewayPayloadLib.writable(bytes32(uint256(0x3333)));
        accounts[3] = SolanaGatewayPayloadLib.readonly(bytes32(uint256(0x4444)));

        bytes memory encoded = SolanaGatewayPayload({executePayload: hex"010203", accounts: accounts}).encodeBorsh();

        bytes memory expected = bytes.concat(
            hex"00",
            hex"03000000",
            hex"010203",
            hex"04000000",
            bytes32(uint256(0x1111)),
            hex"03",
            bytes32(uint256(0x2222)),
            hex"01",
            bytes32(uint256(0x3333)),
            hex"02",
            bytes32(uint256(0x4444)),
            hex"00"
        );

        assertBytesEq(encoded, expected);
    }

    function testAbiEncodingCanBeDecodedAfterSchemeByte() public pure {
        SolanaAccountRepr[] memory accounts = new SolanaAccountRepr[](2);
        accounts[0] = SolanaGatewayPayloadLib.writable(bytes32(uint256(0xaaaa)));
        accounts[1] = SolanaGatewayPayloadLib.signer(bytes32(uint256(0xbbbb)), true);

        bytes memory encoded = SolanaGatewayPayload({executePayload: hex"cafe", accounts: accounts}).encodeAbi();
        require(uint8(encoded[0]) == 1, "bad scheme");

        (bytes memory executePayload, SolanaAccountRepr[] memory decodedAccounts) =
            abi.decode(skipSchemeByte(encoded), (bytes, SolanaAccountRepr[]));

        assertBytesEq(executePayload, hex"cafe");
        require(decodedAccounts.length == 2, "bad account length");
        require(decodedAccounts[0].pubkey == bytes32(uint256(0xaaaa)), "bad account 0 pubkey");
        require(!decodedAccounts[0].isSigner, "bad account 0 signer");
        require(decodedAccounts[0].isWritable, "bad account 0 writable");
        require(decodedAccounts[1].pubkey == bytes32(uint256(0xbbbb)), "bad account 1 pubkey");
        require(decodedAccounts[1].isSigner, "bad account 1 signer");
        require(decodedAccounts[1].isWritable, "bad account 1 writable");
    }

    function testGenericEncodeAndPayloadHash() public pure {
        SolanaAccountRepr[] memory accounts = new SolanaAccountRepr[](1);
        accounts[0] = SolanaGatewayPayloadLib.readonly(bytes32(uint256(0x1234)));

        SolanaGatewayPayload memory payload = SolanaGatewayPayload({executePayload: bytes("memo"), accounts: accounts});

        bytes memory borshEncoded = payload.encode(SolanaGatewayEncoding.Borsh);
        bytes memory abiEncoded = payload.encode(SolanaGatewayEncoding.Abi);

        require(uint8(borshEncoded[0]) == 0, "bad borsh scheme");
        require(uint8(abiEncoded[0]) == 1, "bad abi scheme");
        require(SolanaGatewayPayloadLib.payloadHash(borshEncoded) == keccak256(borshEncoded), "bad hash");
    }

    function testEmitBorshPayloadForRustCompatibility() public {
        SolanaAccountRepr[] memory accounts = new SolanaAccountRepr[](4);
        accounts[0] = SolanaGatewayPayloadLib.account(bytes32(uint256(0x1111)), true, true);
        accounts[1] = SolanaGatewayPayloadLib.signer(bytes32(uint256(0x2222)), false);
        accounts[2] = SolanaGatewayPayloadLib.writable(bytes32(uint256(0x3333)));
        accounts[3] = SolanaGatewayPayloadLib.readonly(bytes32(uint256(0x4444)));

        bytes memory encoded = SolanaGatewayPayload({executePayload: hex"010203", accounts: accounts}).encodeBorsh();

        emit EncodedBorshPayload(encoded);
    }

    function testFuzzBorshEncodingSingleAccount(
        bytes memory executePayload,
        bytes32 pubkey,
        bool isSigner,
        bool isWritable
    ) public pure {
        if (executePayload.length > 512) return;

        SolanaAccountRepr[] memory accounts = new SolanaAccountRepr[](1);
        accounts[0] = SolanaGatewayPayloadLib.account(pubkey, isSigner, isWritable);

        bytes memory encoded = SolanaGatewayPayload({executePayload: executePayload, accounts: accounts}).encodeBorsh();
        bytes memory expected = bytes.concat(
            hex"00",
            le32(uint32(executePayload.length)),
            executePayload,
            le32(1),
            pubkey,
            bytes1((isSigner ? uint8(1) : uint8(0)) | (isWritable ? uint8(2) : uint8(0)))
        );

        assertBytesEq(encoded, expected);
    }

    function testFuzzBorshEncodingMultipleAccounts(
        bytes memory executePayload,
        bytes32[16] memory pubkeys,
        uint16 signerMask,
        uint16 writableMask,
        uint8 accountCount
    ) public pure {
        if (executePayload.length > 512) return;
        accountCount = uint8(bound(accountCount, 0, 16));

        SolanaAccountRepr[] memory accounts = new SolanaAccountRepr[](accountCount);
        for (uint256 i; i < accountCount; ++i) {
            accounts[i] = SolanaGatewayPayloadLib.account(
                pubkeys[i], ((signerMask >> i) & 1) == 1, ((writableMask >> i) & 1) == 1
            );
        }

        bytes memory encoded = SolanaGatewayPayload({executePayload: executePayload, accounts: accounts}).encodeBorsh();

        require(encoded.length == 1 + 4 + executePayload.length + 4 + uint256(accountCount) * 33, "bad encoded length");
        require(uint8(encoded[0]) == 0, "bad scheme");
        assertBytesEq(slice(encoded, 1, 4), le32(uint32(executePayload.length)));
        assertBytesEq(slice(encoded, 5, executePayload.length), executePayload);

        uint256 accountsOffset = 5 + executePayload.length;
        assertBytesEq(slice(encoded, accountsOffset, 4), le32(accountCount));
        accountsOffset += 4;

        for (uint256 i; i < accountCount; ++i) {
            bool isSigner = ((signerMask >> i) & 1) == 1;
            bool isWritable = ((writableMask >> i) & 1) == 1;
            uint8 flags = (isSigner ? uint8(1) : uint8(0)) | (isWritable ? uint8(2) : uint8(0));

            assertBytesEq(slice(encoded, accountsOffset + i * 33, 32), abi.encodePacked(pubkeys[i]));
            require(uint8(encoded[accountsOffset + i * 33 + 32]) == flags, "bad account flags");
        }
    }

    function testFuzzAbiEncodingRoundTrip(bytes memory executePayload, bytes32 pubkey, bool isSigner, bool isWritable)
        public
        pure
    {
        if (executePayload.length > 512) return;

        SolanaAccountRepr[] memory accounts = new SolanaAccountRepr[](1);
        accounts[0] = SolanaGatewayPayloadLib.account(pubkey, isSigner, isWritable);

        bytes memory encoded = SolanaGatewayPayload({executePayload: executePayload, accounts: accounts}).encodeAbi();

        require(uint8(encoded[0]) == 1, "bad scheme");

        (bytes memory decodedPayload, SolanaAccountRepr[] memory decodedAccounts) =
            abi.decode(skipSchemeByte(encoded), (bytes, SolanaAccountRepr[]));

        assertBytesEq(decodedPayload, executePayload);
        require(decodedAccounts.length == 1, "bad account length");
        require(decodedAccounts[0].pubkey == pubkey, "bad pubkey");
        require(decodedAccounts[0].isSigner == isSigner, "bad signer");
        require(decodedAccounts[0].isWritable == isWritable, "bad writable");
    }

    function testFuzzGenericEncodeDispatch(bytes memory executePayload, bytes32 pubkey) public pure {
        if (executePayload.length > 512) return;

        SolanaAccountRepr[] memory accounts = new SolanaAccountRepr[](1);
        accounts[0] = SolanaGatewayPayloadLib.writable(pubkey);

        SolanaGatewayPayload memory payload = SolanaGatewayPayload({executePayload: executePayload, accounts: accounts});

        assertBytesEq(payload.encode(SolanaGatewayEncoding.Borsh), payload.encodeBorsh());
        assertBytesEq(payload.encode(SolanaGatewayEncoding.Abi), payload.encodeAbi());
    }

    function le32(uint32 value) private pure returns (bytes memory) {
        return abi.encodePacked(
            bytes1(uint8(value)), bytes1(uint8(value >> 8)), bytes1(uint8(value >> 16)), bytes1(uint8(value >> 24))
        );
    }

    function bound(uint8 value, uint8 min, uint8 max) private pure returns (uint8) {
        return uint8(uint256(min) + (uint256(value) % (uint256(max) - uint256(min) + 1)));
    }

    function skipSchemeByte(bytes memory input) private pure returns (bytes memory output) {
        require(input.length > 0, "empty input");
        output = new bytes(input.length - 1);
        for (uint256 i; i < output.length; ++i) {
            output[i] = input[i + 1];
        }
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
