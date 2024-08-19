// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

interface MavPool {
    struct SwapParams {
        uint256 amount;
        bool tokenAIn;
        bool exactOutput;
        int32 tickLimit;
    }

    function swap(address recipient, SwapParams memory params, bytes calldata data) external returns (uint256 amountIn, uint256 amountOut);
}

interface IERC20 {}

contract MavQuoter {
    receive() external payable {}

    function maverickV2SwapCallback(IERC20, uint256 amountIn, uint256 amountOut, bytes calldata) external pure {
        revert(string(abi.encode(amountIn, amountOut)));
    }

    function getAmountOut(
        address pool,
        bool zeroForOne,
        uint256 amountIn
    ) external {
        int32 tickLimit = zeroForOne ? type(int32).max : type(int32).min;
        MavPool.SwapParams memory params = MavPool.SwapParams({
            amount: amountIn,
            tokenAIn: zeroForOne,
            exactOutput: false,
            tickLimit: tickLimit
        });

        MavPool(pool).swap(
            address(1),
            params,
            hex"00"
        );
    }
}