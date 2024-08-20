// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import "@aave/core-v3/contracts/flashloan/base/FlashLoanSimpleReceiverBase.sol";
import "@aave/core-v3/contracts/interfaces/IPool.sol";
import "@uniswap/v2-periphery/contracts/interfaces/IUniswapV2Router02.sol";
import "./ISwappers.sol";
import "@balancer-labs/v2-vault/interfaces/contracts/vault/IVault.sol";
import "@balancer-labs/v2-vault/interfaces/contracts/vault/IAsset.sol";
import "@balancer-labs/v2-vault/interfaces/contracts/vault/IBasePool.sol";

contract FlashSwap is FlashLoanSimpleReceiverBase {
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
    address constant BALANCER_VAULT = 0xBA12222222228d8Ba445958a75a0704d566BF2C8;

    address public immutable owner;

    event Profit(uint256 value);

    constructor() FlashLoanSimpleReceiverBase(IPoolAddressesProvider(AAVE_ADDRESSES_PROVIDER) ){
        owner = msg.sender;
    }

    function executeArbitrage(SwapStep[] calldata steps, uint256 amount) external {
        address asset = steps[0].tokenIn;
        bytes memory params = abi.encode(steps, msg.sender);
        POOL.flashLoanSimple(address(this), asset, amount, params, 0);
    }

    function executeOperation(
        address asset,
        uint256 amount,
        uint256 premium,
        address initiator,
        bytes calldata params
    ) external override returns (bool) {
        require(msg.sender == address(POOL), "Caller must be Aave Pool");

        (SwapStep[] memory steps, address caller) = abi.decode(params,(SwapStep[], address));

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
            } else if (step.protocol == 8) {
                _swapSlipstream(step.tokenIn, step.tokenOut, balanceBefore, step.poolAddress, SLIPSTREAM_ROUTER);
            } else if (step.protocol == 9) {
                _swapAerodome(step.poolAddress, step.tokenIn, step.tokenOut, balanceBefore, AERODOME_ROUTER);
            } if (step.protocol == 10) {
                _swapBalancer(step.poolAddress, step.tokenIn, step.tokenOut, balanceBefore);
            } else {

                revert("Invalid protocol");
            }
            amountIn = IERC20(step.tokenOut).balanceOf(address(this));
        }

        uint256 amountToRepay = amount + premium;
        uint256 finalBalance = IERC20(asset).balanceOf(address(this));
        IERC20(asset).approve(address(POOL), finalBalance);
        require(finalBalance >= amountToRepay, "Insufficient balance to repay flash loan");
        
        uint256 profit = finalBalance - amountToRepay;
        if (profit > 0) {
            IERC20(asset).transfer(caller, profit);
            emit Profit(profit);
        }
        return true;
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
        bool stable = AerodromePool(poolAddress).stable();
        IAerodrome.Route[] memory routes = new IAerodrome.Route[](1);
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

    function _swapBalancer(address poolAddress, address tokenIn, address tokenOut, uint256 amountIn) internal {
        // 1. Get the pool ID from the pool address
        bytes32 poolId = IBasePool(poolAddress).getPoolId();

        // 2. Approve the Balancer Vault to spend tokens
        IERC20(tokenIn).approve(BALANCER_VAULT, amountIn);

        // 3. Perform the swap
        IVault(BALANCER_VAULT).swap(
            IVault.SingleSwap({
                poolId: poolId,
                kind: IVault.SwapKind.GIVEN_IN,
                assetIn: IAsset(tokenIn),
                assetOut: IAsset(tokenOut),
                amount: amountIn,
                userData: ""
            }),
            IVault.FundManagement({
                sender: address(this),
                fromInternalBalance: false,
                recipient: payable(address(this)),
                toInternalBalance: false
            }),
            0, // Minimum amount out (we're not enforcing a minimum in this example)
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

    function withdraw(address token) external {
        require(msg.sender == owner, "Only owner can withdraw");
        uint256 balance = IERC20(token).balanceOf(address(this));
        IERC20(token).transfer(owner, balance);
    }
}
