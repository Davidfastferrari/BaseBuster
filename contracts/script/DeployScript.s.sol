// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import "forge-std/Script.sol";
import "../src/FlashSwap.sol";

contract DeployFlashSwap is Script {
    function run() external {
        uint256 deployerPrivateKey = vm.envUint("PRIVATE_KEY");
        address[] memory routers = new address[](11);
        routers[0] = 0x4752ba5DBc23f44D87826276BF6Fd6b1C372aD24; // UNISWAP_V2_ROUTER
        routers[1] = 0x6BDED42c6DA8FBf0d2bA55B2fa120C5e0c8D7891; // SUSHISWAP_V2_ROUTER
        routers[2] = 0x8cFe327CEc66d1C090Dd72bd0FF11d690C33a2Eb; // PANCAKESWAP_V2_ROUTER
        routers[3] = 0x327Df1E6de05895d2ab08513aaDD9313Fe505d86; // BASESWAP_V2_ROUTER
        routers[4] = 0x2626664c2603336E57B271c5C0b26F421741e481; // UNISWAP_V3_ROUTER
        routers[5] = 0x678Aa4bF4E210cf2166753e054d5b7c31cc7fa86; // PANCAKESWAP_V3_ROUTER
        routers[6] = 0xFB7eF66a7e61224DD6FcD0D7d9C3be5C8B049b9f; // SUSHISWAP_V3_ROUTER
        routers[7] = 0x1B8eea9315bE495187D873DA7773a874545D9D48; // BASESWAP_V3_ROUTER
        routers[8] = 0xBE6D8f0d05cC4be24d5167a3eF062215bE6D18a5; // SLIPSTREAM_ROUTER
        routers[9] = 0xcF77a3Ba9A5CA399B7c97c74d54e5b1Beb874E43; // AERODOME_ROUTER
        routers[10] = 0xBA12222222228d8Ba445958a75a0704d566BF2C8; // BALANCER_VAULT

        vm.startBroadcast(deployerPrivateKey);

        FlashSwap flashSwap = new FlashSwap(0xe20fCBdBfFC4Dd138cE8b2E6FBb6CB49777ad64D, routers);

        console.log("FlashSwap deployed at:", address(flashSwap));

        vm.stopBroadcast();
    }
}