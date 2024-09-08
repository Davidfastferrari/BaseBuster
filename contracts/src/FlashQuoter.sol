// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

interface IERC20 {
    function balanceOf(address account) external view returns (uint256);
    function transfer(address recipient, uint256 amount) external returns (bool);
    function transferFrom(address sender, address recipient, uint256 amount) external returns (bool);
    function approve(address spender, uint256 amount) external returns (bool);
}

interface IWETH is IERC20 {
    function deposit() external payable;
    function withdraw(uint256 amount) external;
}

interface IUniswapV2Router {
    function swapExactTokensForTokens(
        uint256 amountIn,
        uint256 amountOutMin,
        address[] calldata path,
        address to,
        uint256 deadline
    ) external returns (uint256[] memory amounts);
}

interface IV3SwapRouterWithDeadline {
    struct ExactInputSingleParams {
        address tokenIn;
        address tokenOut;
        uint24 fee;
        address recipient;
        uint256 deadline;
        uint256 amountIn;
        uint256 amountOutMinimum;
        uint160 sqrtPriceLimitX96;
    }
    function exactInputSingle(ExactInputSingleParams calldata params) external returns (uint256 amountOut);
}

interface IV3SwapRouterNoDeadline {
    struct ExactInputSingleParams {
        address tokenIn;
        address tokenOut;
        uint24 fee;
        address recipient;
        uint256 amountIn;
        uint256 amountOutMinimum;
        uint160 sqrtPriceLimitX96;
    }
    function exactInputSingle(ExactInputSingleParams calldata params) external returns (uint256 amountOut);
}

interface ISlipstream {
    struct ExactInputSingleParams {
        address tokenIn;
        address tokenOut;
        int24 tickSpacing;
        address recipient;
        uint256 deadline;
        uint256 amountIn;
        uint256 amountOutMinimum;
        uint160 sqrtPriceLimitX96;
    }
    function exactInputSingle(ExactInputSingleParams calldata params) external payable returns (uint256 amountOut);
}

interface ISlipstreamPool {
    function tickSpacing() external view returns (int24);
}


interface IAerodromeRouter {
    struct Route {
        address from;
        address to;
        bool stable;
        address factory;
    }
    function swapExactTokensForTokens(
        uint256 amountIn,
        uint256 amountOutMin,
        Route[] calldata routes,
        address to,
        uint256 deadline
    ) external returns (uint256[] memory amounts);
}

interface IAerodromePool {
    function stable() external view returns (bool);
    function factory() external view returns (address);
}

interface IBalancerVault {
    enum SwapKind { GIVEN_IN, GIVEN_OUT }
    struct SingleSwap {
        bytes32 poolId;
        SwapKind kind;
        address assetIn;
        address assetOut;
        uint256 amount;
        bytes userData;
    }
    struct FundManagement {
        address sender;
        bool fromInternalBalance;
        address payable recipient;
        bool toInternalBalance;
    }
    function swap(
        SingleSwap memory singleSwap,
        FundManagement memory funds,
        uint256 limit,
        uint256 deadline
    ) external returns (uint256);
}

interface IBalancerPool {
    function getPoolId() external view returns (bytes32);
}


contract FlashQuoter {
    struct SwapStep {
        address poolAddress;
        address tokenIn;
        address tokenOut;
        uint8 protocol;
        uint24 fee;
    }

    // Router addresses group by interface/swap method
    // V2 VARIATIONS
    address constant UNISWAP_V2_ROUTER = 0x4752ba5DBc23f44D87826276BF6Fd6b1C372aD24; 
    address constant SUSHISWAP_V2_ROUTER = 0x6BDED42c6DA8FBf0d2bA55B2fa120C5e0c8D7891; 
    address constant PANCAKESWAP_V2_ROUTER = 0x8cFe327CEc66d1C090Dd72bd0FF11d690C33a2Eb; 
    address constant BASESWAP_V2_ROUTER = 0x327Df1E6de05895d2ab08513aaDD9313Fe505d86; 
    address constant SWAPBASED_V2_ROUTER = 0xaaa3b1F1bd7BCc97fD1917c18ADE665C5D31F066;
    address constant ALIENBASE_V2_ROUTER = 0x8c1A3cF8f83074169FE5D7aD50B978e1cD6b37c7;
    address constant DACKIESWAP_V2_ROUTER = 0xCa4EAa32E7081b0c4Ba47e2bDF9B7163907Fe56f;

    // V3 VARIATION

    // NODEADLINE
    address constant UNISWAP_V3_ROUTER = 0x2626664c2603336E57B271c5C0b26F421741e481; 
    address constant ALIENBASE_V3_ROUTER = 0xB20C411FC84FBB27e78608C24d0056D974ea9411;
    address constant DACKIESWAP_V3_ROUTER = 0x195FBc5B8Fbd5Ac739C1BA57D4Ef6D5a704F34f7;
    address constant PANCAKESWAP_V3_ROUTER = 0x678Aa4bF4E210cf2166753e054d5b7c31cc7fa86; 

    // DEADLINE
    address constant SUSHISWAP_V3_ROUTER = 0xFB7eF66a7e61224DD6FcD0D7d9C3be5C8B049b9f; 
    address constant SWAPBASED_V3_ROUTER = 0x756C6BbDd915202adac7beBB1c6C89aC0886503f;
    address constant BASESWAP_V3_ROUTER = 0x1B8eea9315bE495187D873DA7773a874545D9D48; 

    // SLIPSTREAM
    address constant SLIPSTREAM_ROUTER = 0xBE6D8f0d05cC4be24d5167a3eF062215bE6D18a5;

    // AERODROME
    address constant AERODOME_ROUTER = 0xcF77a3Ba9A5CA399B7c97c74d54e5b1Beb874E43; 

    // BALANCER
    address constant BALANCER_VAULT = 0xBA12222222228d8Ba445958a75a0704d566BF2C8;


    // TOADD
    // Mavirkc v1, v2, curve two and tri

    // FLASHLOAN
    address constant AAVE_ADDRESSES_PROVIDER = 0xe20fCBdBfFC4Dd138cE8b2E6FBb6CB49777ad64D;
    IWETH public constant WETH = IWETH(0x4200000000000000000000000000000000000006);

    function quoteArbitrage(SwapStep[] calldata steps, uint256 amount) external returns (uint256) {
        require(steps.length > 0, "Invalid path");
        require(amount > 0, "Invalid amount");

        // Transfer WETH from sender to this contract
        require(WETH.transferFrom(msg.sender, address(this), amount), "WETH transfer failed");

        uint256 amountIn = amount;

        for (uint256 i = 0; i < steps.length; i++) {
            uint256 balance = IERC20(steps[i].tokenIn).balanceOf(address(this));
            IERC20(steps[i].tokenIn).approve(_getRouter(steps[i].protocol), balance);
            amountIn = _swap(steps[i], balance);
        }
        return amountIn;
    }

    function _getRouter(uint8 protocol) private pure returns (address) {
        // V2 Variants
        if (protocol == 0) return UNISWAP_V2_ROUTER;
        if (protocol == 1) return SUSHISWAP_V2_ROUTER;
        if (protocol == 2) return PANCAKESWAP_V2_ROUTER;
        if (protocol == 3) return BASESWAP_V2_ROUTER;
        if (protocol == 4) return SWAPBASED_V2_ROUTER;
        if (protocol == 5) return ALIENBASE_V2_ROUTER;
        if (protocol == 6) return DACKIESWAP_V2_ROUTER;

        // V3 Variants (No Deadline)
        if (protocol == 7) return UNISWAP_V3_ROUTER;
        if (protocol == 8) return ALIENBASE_V3_ROUTER;
        if (protocol == 9) return DACKIESWAP_V3_ROUTER;
        if (protocol == 10) return PANCAKESWAP_V3_ROUTER;

        // V3 Variants (With Deadline)
        if (protocol == 11) return SUSHISWAP_V3_ROUTER;
        if (protocol == 12) return SWAPBASED_V3_ROUTER;
        if (protocol == 13) return BASESWAP_V3_ROUTER;

        // Other protocols
        if (protocol == 14) return SLIPSTREAM_ROUTER;
        if (protocol == 15) return AERODOME_ROUTER;
        if (protocol == 16) return BALANCER_VAULT;

        // TODO: Implement these protocols
        // if (protocol == 17) return MAVERICK_V1_ROUTER;
        // if (protocol == 18) return MAVERICK_V2_ROUTER;
        // if (protocol == 19) return CURVE_TWO_CRYPTO_ROUTER;
        // if (protocol == 20) return CURVE_TRI_CRYPTO_ROUTER;

        revert("Invalid protocol");
    }

    function _swap(SwapStep memory step, uint256 amountIn) private returns (uint256) {
        if (step.protocol <= 6) {
            return _swapV2(step, amountIn);
        } else if (step.protocol <= 13) {
            return _swapV3(step, amountIn);
        } else if (step.protocol == 14) {
            return _swapSlipstream(step, amountIn);
        } else if (step.protocol == 15) {
            return _swapAerodrome(step, amountIn);
        } else if (step.protocol == 16) {
            return _swapBalancer(step, amountIn);
        } else {
            revert("Unsupported protocol");
        }
    }

    function _swapV2(SwapStep memory step, uint256 amountIn) private returns (uint256) {
        address[] memory path = new address[](2);
        path[0] = step.tokenIn;
        path[1] = step.tokenOut;
        uint256[] memory amounts = IUniswapV2Router(_getRouter(step.protocol)).swapExactTokensForTokens(
            amountIn, 0, path, address(this), block.timestamp
        );
        require(amounts.length > 1, "Invalid swap result");
        return amounts[1];
    }

    function _swapV3(SwapStep memory step, uint256 amountIn) private returns (uint256) {
        if (step.protocol <= 10 ) {
            return IV3SwapRouterNoDeadline(_getRouter(step.protocol)).exactInputSingle(
                IV3SwapRouterNoDeadline.ExactInputSingleParams({
                    tokenIn: step.tokenIn,
                    tokenOut: step.tokenOut,
                    fee: step.fee,
                    recipient: address(this),
                    amountIn: amountIn,
                    amountOutMinimum: 0,
                    sqrtPriceLimitX96: 0
                })
            );
        } else {
            return IV3SwapRouterWithDeadline(_getRouter(step.protocol)).exactInputSingle(
                IV3SwapRouterWithDeadline.ExactInputSingleParams({
                    tokenIn: step.tokenIn,
                    tokenOut: step.tokenOut,
                    fee: step.fee,
                    recipient: address(this),
                    deadline: block.timestamp,
                    amountIn: amountIn,
                    amountOutMinimum: 0,
                    sqrtPriceLimitX96: 0
                })
            );
        }
    }

    function _swapSlipstream(SwapStep memory step, uint256 amountIn) private returns (uint256) {
        int24 tick_spacing = ISlipstreamPool(step.poolAddress).tickSpacing();
        return ISlipstream(_getRouter(step.protocol)).exactInputSingle(
           ISlipstream.ExactInputSingleParams({
               tokenIn: step.tokenIn,
               tokenOut: step.tokenOut,
               tickSpacing: tick_spacing,
               recipient: address(this),
               deadline: block.timestamp,
               amountIn: amountIn,
               amountOutMinimum: 0,
               sqrtPriceLimitX96: 0
           })
        );
    }

    function _swapAerodrome(SwapStep memory step, uint256 amountIn) private returns (uint256) {
        IAerodromeRouter.Route[] memory routes = new IAerodromeRouter.Route[](1);
        bool isStable = IAerodromePool(step.poolAddress).stable();
        address factoryAddr = IAerodromePool(step.poolAddress).factory();
        routes[0] = IAerodromeRouter.Route({
            from: step.tokenIn,
            to: step.tokenOut,
            stable: isStable,
            factory: address(0)
        });
        uint256[] memory amounts = IAerodromeRouter(_getRouter(step.protocol)).swapExactTokensForTokens(
            amountIn, 0, routes, address(this), block.timestamp
        );
        require(amounts.length > 1, "Invalid swap result");
        return amounts[1];
    }
    function _swapBalancer(SwapStep memory step, uint256 amountIn) private returns (uint256) {
        bytes32 poolId = IBalancerPool(step.poolAddress).getPoolId(); 
        return IBalancerVault(_getRouter(step.protocol)).swap(
            IBalancerVault.SingleSwap({
                poolId: poolId,
                kind: IBalancerVault.SwapKind.GIVEN_IN,
                assetIn: step.tokenIn,
                assetOut: step.tokenOut,
                amount: amountIn,
                userData: ""
            }),
            IBalancerVault.FundManagement({
                sender: address(this),
                fromInternalBalance: false,
                recipient: payable(address(this)),
                toInternalBalance: false
            }),
            0,
            block.timestamp
        );
}

    // Function to rescue tokens sent to the contract by mistake
    function rescueTokens(address token) external {
        uint256 balance = IERC20(token).balanceOf(address(this));
        require(balance > 0, "No tokens to rescue");
        IERC20(token).transfer(msg.sender, balance);
    }

    // Function to receive ETH
    receive() external payable {}
}