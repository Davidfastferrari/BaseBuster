use alloy::sol;

sol!(
    #[derive(Debug)]
    contract AerodromeEvent {
        event Sync(uint256 reserve0, uint256 reserve1);
    }
);

sol!(
    #[derive(Debug)]
    #[sol(rpc)]
    contract PancakeSwap {
        event Swap(
            address indexed sender,
            address indexed recipient,
            int256 amount0,
            int256 amount1,
            uint160 sqrtPriceX96,
            uint128 liquidity,
            int24 tick,
            uint128 protocolFeesToken0,
            uint128 protocolFeesToken1
        );
    }
);
sol! {
    #[derive(Debug)]
    contract BalancerV2Event {
        event Swap(
            bytes32 indexed poolId,
            address indexed tokenIn,
            address indexed tokenOut,
            uint256 amountIn,
            uint256 amountOut
        );
    }
}


sol! {
    #[derive(Debug)]
    
    contract DataEvent {
        event Sync(uint112 reserve0, uint112 reserve1);
        event Swap(
            address indexed sender,
            address indexed recipient,
            int256 amount0,
            int256 amount1,
            uint160 sqrtPriceX96,
            uint128 liquidity,
            int24 tick
        );

        event Mint(
            address sender,
            address indexed owner,
            int24 indexed tickLower,
            int24 indexed tickUpper,
            uint128 amount,
            uint256 amount0,
            uint256 amount1
        );

        event Burn(
            address indexed owner,
            int24 indexed tickLower,
            int24 indexed tickUpper,
            uint128 amount,
            uint256 amount0,
            uint256 amount1
        );
    }
}

// define our flash swap contract
sol!(
    #[derive(Debug)]
    #[sol(rpc)]
    FlashSwap,
    "src/abi/FlashSwap.json"
);
