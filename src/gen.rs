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

sol!(
    #[derive(Debug)]
    #[sol(rpc)]
    FlashQuoter,
    "src/abi/FlashQuoter.json"
);

// Abi Generation an ERC20 token
sol!(
    #[sol(rpc)]
    contract ERC20Token {
        function totalSupply() external view returns (uint256 totalSupply);
        function balanceOf(address account) external view returns (uint256 balance);
        function symbol() external view returns (string memory symbol);
        function approve(address spender, uint256 amount) external returns (bool success);
        function allowance(address owner, address spender) public view returns (uint256 allowance);
        function decimals() public view returns (uint8 decimals);
        function deposit() external payable;
        function transferFrom(address from, address to, uint256 amount) external returns (bool success);
    }
);
