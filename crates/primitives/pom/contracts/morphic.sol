// SPDX-License-Identifier: Apache-2.0
pragma solidity >=0.8.25;

contract mp {
    // Add event definition
    event ChallengeEvent(bytes data);

    enum QuoteState {
        DEFAULT,
        FAILED,
        PASSED
    }
    struct TEENodeInfo {
        uint32 quoteSize;
        bytes quoteBuf;
        uint32 supSize;
        bytes supBuf;
        string teePublicKey;
        string p2pConnectInfo; //e.g. ip4/7.7.7.7/tcp/4242/p2p/QmYyQSo1c1Ym7orWxLYvCrM2EmxFTANf8wXmmE7DWjhx5N
        address operator;
        QuoteState quoteState;
        string appAddr;
    }
    mapping(string => TEENodeInfo) teeRegMap; // key: peerId
    string[] teeRegList;

    function registerTEE(
        string memory peerId,
        uint32 quoteSize,
        bytes calldata quoteBuf,
        uint32 supSize,
        bytes calldata supBuf,
        string calldata teePublicKey,
        string calldata p2pConnectInfo,
        string calldata appAddr
    ) external {
        TEENodeInfo storage teeNodeInfo = teeRegMap[peerId];
        require(teeNodeInfo.operator == address(0), "TEE registered already");
        teeNodeInfo.quoteSize = quoteSize;
        teeNodeInfo.quoteBuf = quoteBuf;
        teeNodeInfo.supSize = supSize;
        teeNodeInfo.supBuf = supBuf;
        teeNodeInfo.teePublicKey = teePublicKey;
        teeNodeInfo.p2pConnectInfo = p2pConnectInfo;
        teeNodeInfo.quoteState = QuoteState.PASSED;
        teeNodeInfo.appAddr = appAddr;
        teeNodeInfo.operator = msg.sender;
        teeRegList.push(peerId);
    }

    function deleteTEE(string calldata peerId) external {
        TEENodeInfo storage teeNodeInfo = teeRegMap[peerId];
        require(
            teeNodeInfo.operator == msg.sender,
            "Permission denied: not operator"
        );
        for (uint i = 0; i < teeRegList.length; i++) {
            if (
                keccak256(abi.encodePacked(teeRegList[i])) ==
                keccak256(abi.encodePacked(peerId))
            ) {
                teeRegList[i] = teeRegList[teeRegList.length - 1];
                teeRegList.pop();
                break;
            }
        }
        teeNodeInfo.quoteSize = 0;
        teeNodeInfo.quoteBuf = new bytes(0);
        teeNodeInfo.supSize = 0;
        teeNodeInfo.supBuf = new bytes(0);
        teeNodeInfo.teePublicKey = "";
        teeNodeInfo.p2pConnectInfo = "";
        teeNodeInfo.quoteState = QuoteState.DEFAULT;
        teeNodeInfo.operator = address(0);
    }

    function getQuote(
        string calldata peerId
    ) external view returns (uint32, bytes memory, uint32, bytes memory) {
        TEENodeInfo memory teeNodeInfo = teeRegMap[peerId];
        return (
            teeNodeInfo.quoteSize,
            teeNodeInfo.quoteBuf,
            teeNodeInfo.supSize,
            teeNodeInfo.supBuf
        );
    }

    struct ApiInfo {
        string appAddr; // Contract address or URL address (for web2 applications)
        string method; // Method
        uint timeout; // Maximum execution time promised by this interface
    }
    mapping(string => ApiInfo) apiInfoMap; // key: appAddr+method

    function registerApi(
        string calldata peerId,
        string calldata appAddr,
        string calldata method,
        uint timeout
    ) external {
        TEENodeInfo storage teeNodeInfo = teeRegMap[peerId];
        require(
            teeNodeInfo.operator == msg.sender,
            "Permission denied: not operator"
        );
        require(
            keccak256(abi.encodePacked(teeNodeInfo.appAddr)) ==
                keccak256(abi.encodePacked(appAddr)),
            "Permission denied: node not belong to the app"
        );
        ApiInfo storage apiInfo = apiInfoMap[
            string(abi.encodePacked(appAddr, method))
        ];
        apiInfo.appAddr = appAddr;
        apiInfo.method = method;
        apiInfo.timeout = timeout;
    }

    // Update the updateChallengeBytes function to properly update challenge data
    function updateChallengeBytes(
        string calldata peerId,
        bytes calldata challengeId,
        bytes calldata data, // abi encoded pom struct
        bytes calldata sig
    ) public {
        // 触发事件
        emit ChallengeEvent(data);
    }

    function verifyTEESig(
        bytes32 data,
        string calldata peerId,
        bytes calldata sig
    ) internal {
        TEENodeInfo storage teeNodeInfo = teeRegMap[peerId];
        require(
            recoverSigner(data, sig) ==
                stringToAddress(teeNodeInfo.teePublicKey),
            "Failed to verify TEE sig"
        );
    }

    function recoverSigner(
        bytes32 message,
        bytes memory sig
    ) internal pure returns (address) {
        (uint8 v, bytes32 r, bytes32 s) = splitSignature(sig);
        return ecrecover(message, v, r, s);
    }

    function splitSignature(
        bytes memory sig
    ) internal pure returns (uint8 v, bytes32 r, bytes32 s) {
        require(sig.length == 65);
        assembly {
            r := mload(add(sig, 32))
            s := mload(add(sig, 64))
            v := byte(0, mload(add(sig, 96)))
        }
        return (v, r, s);
    }

    function stringToAddress(
        string memory str
    ) internal pure returns (address) {
        address addr;
        assembly {
            addr := mload(add(str, 20))
        }
        return addr;
    }
}
