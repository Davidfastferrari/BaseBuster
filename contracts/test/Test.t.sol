// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.0;

import "forge-std/Test.sol";
import "../src/BalancerTester.sol";

contract BalancerTesterTest is Test {
    BalancerTester public tester;

    function setUp() public {
        tester = new BalancerTester();
    }

    function testProcessGeneralPoolSwapRequest() public {
        uint256 balanceIn = 105341662945605553931;
        uint256 balanceOut = 1627354805625147704049411;
        uint256 weightIn = 200000000000000000;
        uint256 weightOut = 800000000000000000;
        uint256 swapFeePercentage = 3000000000000000;

        console.log("Running _processGeneralPoolSwapRequest...");

        uint256 amountCalculated = tester._processGeneralPoolSwapRequest(
            balanceIn,
            balanceOut,
            weightIn,
            weightOut,
            swapFeePercentage
        );

        console.log("Final Amount Calculated:", amountCalculated);

        // Add assertions as needed
        assertTrue(amountCalculated > 0, "Amount calculated should be greater than 0");
    }
}