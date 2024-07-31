// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@uniswap/v2-periphery/contracts/interfaces/IUniswapV2Router02.sol";
import "@uniswap/v3-periphery/contracts/interfaces/ISwapRouter.sol";

contract MultiExchangeSwap {
    enum PoolType { UniswapV2, UniswapV3, SushiswapV2, SushiswapV3, PancakeswapV2 }

    struct SwapStep {
        address poolAddress;
        address tokenIn;
        address tokenOut;
        PoolType protocol;
    }

    address private constant UNISWAP_V2_ROUTER = 0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D;
    address private constant UNISWAP_V3_ROUTER = 0xE592427A0AEce92De3Edee1F18E0157C05861564;
    address private constant SUSHISWAP_V2_ROUTER = 0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F;
    address private constant PANCAKESWAP_V2_ROUTER = 0x10ED43C718714eb63d5aA57B78B54704E256024E;

    function swap(SwapStep[] memory steps, uint256 amountIn) external returns (uint256) {
        require(steps.length > 0, "No swap steps provided");

        IERC20 tokenIn = IERC20(steps[0].tokenIn);
        require(tokenIn.transferFrom(msg.sender, address(this), amountIn), "Transfer failed");

        uint256 currentAmount = amountIn;

        for (uint256 i = 0; i < steps.length; i++) {
            SwapStep memory step = steps[i];
            require(step.tokenIn != address(0) && step.tokenOut != address(0), "Invalid token addresses");

            IERC20(step.tokenIn).approve(step.poolAddress, currentAmount);

            if (step.protocol == PoolType.UniswapV2 || step.protocol == PoolType.SushiswapV2 || step.protocol == PoolType.PancakeswapV2) {
                currentAmount = swapV2(step, currentAmount);
            } else if (step.protocol == PoolType.UniswapV3 || step.protocol == PoolType.SushiswapV3) {
                currentAmount = swapV3(step, currentAmount);
            } else {
                revert("Unsupported protocol");
            }
        }

        IERC20 finalToken = IERC20(steps[steps.length - 1].tokenOut);
        require(finalToken.transfer(msg.sender, currentAmount), "Final transfer failed");

        return currentAmount;
    }

    function swapV2(SwapStep memory step, uint256 amountIn) private returns (uint256) {
        address[] memory path = new address[](2);
        path[0] = step.tokenIn;
        path[1] = step.tokenOut;

        IUniswapV2Router02 router = IUniswapV2Router02(step.poolAddress);
        uint256[] memory amounts = router.swapExactTokensForTokens(
            amountIn,
            0,
            path,
            address(this),
            block.timestamp
        );

        return amounts[amounts.length - 1];
    }

    function swapV3(SwapStep memory step, uint256 amountIn) private returns (uint256) {
        ISwapRouter router = ISwapRouter(step.poolAddress);
        ISwapRouter.ExactInputSingleParams memory params = ISwapRouter.ExactInputSingleParams({
            tokenIn: step.tokenIn,
            tokenOut: step.tokenOut,
            fee: 3000, // Assuming medium fee tier, adjust as needed
            recipient: address(this),
            deadline: block.timestamp,
            amountIn: amountIn,
            amountOutMinimum: 0,
            sqrtPriceLimitX96: 0
        });

        return router.exactInputSingle(params);
    }
}
