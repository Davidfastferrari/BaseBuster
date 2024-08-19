// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import "@aave/core-v3/contracts/interfaces/IPool.sol";
import "@uniswap/v2-periphery/contracts/interfaces/IUniswapV2Router02.sol";
import "./ISwappers.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";

contract FlashQuoter {
    struct SwapStep {
        address poolAddress;
        address tokenIn;
        address tokenOut;
        uint8 protocol;
        uint24 fee;
    }

    // Router addresses group by interface/swap method
    address constant UNISWAP_V2_ROUTER = 0x4752ba5DBc23f44D87826276BF6Fd6b1C372aD24; 
    address constant SUSHISWAP_V2_ROUTER = 0x6BDED42c6DA8FBf0d2bA55B2fa120C5e0c8D7891; 
    address constant PANCAKESWAP_V2_ROUTER = 0x8cFe327CEc66d1C090Dd72bd0FF11d690C33a2Eb; 
    address constant BASESWAP_V2_ROUTER = 0x327Df1E6de05895d2ab08513aaDD9313Fe505d86; 

    address constant UNISWAP_V3_ROUTER = 0x2626664c2603336E57B271c5C0b26F421741e481; 
    address constant PANCAKESWAP_V3_ROUTER = 0x678Aa4bF4E210cf2166753e054d5b7c31cc7fa86; 

    address constant SUSHISWAP_V3_ROUTER = 0xFB7eF66a7e61224DD6FcD0D7d9C3be5C8B049b9f; 
    address constant BASESWAP_V3_ROUTER = 0x1B8eea9315bE495187D873DA7773a874545D9D48; 
    address constant SLIPSTREAM_ROUTER = 0xBE6D8f0d05cC4be24d5167a3eF062215bE6D18a5;
   
    address constant AERODOME_ROUTER = 0xcF77a3Ba9A5CA399B7c97c74d54e5b1Beb874E43; 

    address constant AAVE_ADDRESSES_PROVIDER =0xe20fCBdBfFC4Dd138cE8b2E6FBb6CB49777ad64D;

    address constant WETH = 0x4200000000000000000000000000000000000006;

    event Profit(uint256 value);

    function executeArbitrage(SwapStep[] calldata steps, uint256 amount) external returns (uint256){
        uint256 amountIn = amount;

        for (uint i = 0; i < steps.length; i++) {
            SwapStep memory step = steps[i];
            uint256 balanceBefore = IERC20(step.tokenIn).balanceOf(address(this));

            if (step.protocol <= 3) {
                _swapV2(step.tokenIn, step.tokenOut, balanceBefore, _getV2Router(step.protocol));
            } else if (step.protocol <= 5) {
                _swapV3(step.tokenIn, step.tokenOut, balanceBefore, step.fee, _getV3Router(step.protocol));
            } else if (step.protocol <= 7) {
                _swapV3Deadline(step.tokenIn, step.tokenOut, balanceBefore, step.fee, _getV3RouterDeadline(step.protocol));
            }  else if (step.protocol == 8) {
                 _swapSlipstream(step.tokenIn, step.tokenOut, balanceBefore, step.poolAddress, SLIPSTREAM_ROUTER);
            } else if (step.protocol == 9) {
                _swapAerodome(step.poolAddress, step.tokenIn, step.tokenOut, balanceBefore, AERODOME_ROUTER);
            } else {
                revert("Invalid protocol");
            }
            amountIn = IERC20(step.tokenOut).balanceOf(address(this));
        }

        uint256 finalBalance = IERC20(WETH).balanceOf(address(this));
        return finalBalance;
    }

    function _swapV2(address tokenIn, address tokenOut, uint256 amountIn, address router) internal {
        IERC20(tokenIn).approve(router, amountIn);
        address[] memory path = new address[](2);
        path[0] = tokenIn;
        path[1] = tokenOut;
        IUniswapV2Router02(router).swapExactTokensForTokens(
            amountIn,
            0,
            path,
            address(this),
            block.timestamp
        );
    }

    function _swapV3(address tokenIn, address tokenOut, uint256 amountIn, uint24 fee, address router) internal {
        IERC20(tokenIn).approve(router, amountIn);
        V3NoDeadline.ExactInputSingleParams memory params = V3NoDeadline.ExactInputSingleParams({
            tokenIn: tokenIn,
            tokenOut: tokenOut,
            fee: fee,
            recipient: address(this),
            amountIn: amountIn,
            amountOutMinimum: 0,
            sqrtPriceLimitX96: 0
        });
        V3NoDeadline(router).exactInputSingle(params);
    }

    function _swapV3Deadline(address tokenIn, address tokenOut, uint256 amountIn, uint24 fee, address router) internal {
        IERC20(tokenIn).approve(router, amountIn);
        V3Deadline.ExactInputSingleParams memory params = V3Deadline.ExactInputSingleParams({
            tokenIn: tokenIn,
            tokenOut: tokenOut,
            fee: fee,
            recipient: address(this),
            deadline: block.timestamp + 100,
            amountIn: amountIn,
            amountOutMinimum: 0,
            sqrtPriceLimitX96: 0
        });
        V3Deadline(router).exactInputSingle(params);
    }

    function _swapSlipstream(address tokenIn, address tokenOut, uint256 amountIn, address poolAddress, address router) internal {
        int24 tickSpacing = SlipstreamPool(poolAddress).tickSpacing();
        IERC20(tokenIn).approve(router, amountIn);
        SlipstreamRouter.ExactInputSingleParams memory params = SlipstreamRouter.ExactInputSingleParams({
            tokenIn: tokenIn,
            tokenOut: tokenOut,
            tickSpacing: tickSpacing,
            recipient: address(this),
            deadline: block.timestamp + 100,
            amountIn: amountIn,
            amountOutMinimum: 0,
            sqrtPriceLimitX96: 0
        });
        SlipstreamRouter(router).exactInputSingle(params);
    }


    function _swapAerodome(address poolAddress, address tokenIn, address tokenOut, uint256 amountIn, address router) internal {
        IERC20(tokenIn).approve(router, amountIn);
        IAerodrome.Route[] memory routes = new IAerodrome.Route[](1);
        bool stable = AerodromePool(poolAddress).stable();
        routes[0] = IAerodrome.Route({
            from: tokenIn,
            to: tokenOut,
            stable: stable,
            factory: address(0)
        });
        IAerodrome(router).swapExactTokensForTokens(
            amountIn,
            0,
            routes,
            address(this),
            block.timestamp
        );
    }

    function _getV2Router(uint8 protocol) internal pure returns (address) {
        if (protocol == 0) return UNISWAP_V2_ROUTER;
        if (protocol == 1) return SUSHISWAP_V2_ROUTER;
        if (protocol == 2) return PANCAKESWAP_V2_ROUTER;
        if (protocol == 3) return BASESWAP_V2_ROUTER;
        revert("Invalid V2 protocol");
    }

    function _getV3Router(uint8 protocol) internal pure returns (address) {
        if (protocol == 4) return UNISWAP_V3_ROUTER;
        if (protocol == 5) return PANCAKESWAP_V3_ROUTER;
        revert("Invalid V3 protocol");
    }

    function _getV3RouterDeadline(uint8 protocol) internal pure returns (address) {
        if (protocol == 6) return SUSHISWAP_V3_ROUTER;
        if (protocol == 7) return BASESWAP_V3_ROUTER;
        if (protocol == 8) return SLIPSTREAM_ROUTER;
        revert("Invalid V3 protocol");
    }

    receive() external payable {}
}
