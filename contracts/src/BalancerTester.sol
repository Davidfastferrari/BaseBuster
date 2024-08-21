// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.0;

import "./LogExp.sol";
import "forge-std/console.sol";

contract BalancerTester {
    using LogExpMath for uint256;
    using Math for uint256;
    using FixedPoint for uint256;

    function _processGeneralPoolSwapRequest(
        uint256 balanceIn,
        uint256 balanceOut,
        uint256 weightIn,
        uint256 weightOut,
        uint256 swapFeePercentage
    )
    external
    view
    returns (uint256 amountCalculated)
    {
        uint256 amount_in = 9970000000000000;
        //console.log("amount_in", amount_in);
        //console.log("balanceIn", balanceIn);
        uint256 denominator = balanceIn + amount_in;
        //console.log("denominator", denominator);
        uint256 base = balanceIn.divUp(denominator);
        //console.log("base", base);
        uint256 exponent = weightIn.divDown(weightOut);
        //console.log("exponent", exponent);
        uint256 power = base.powUp(exponent);
        //console.log("power", power);

        uint256 result = balanceOut.mulDown(power.complement());
        //console.log("result", result);

        amountCalculated = 10;

    }
}