// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import "@aave/core-v3/contracts/flashloan/base/FlashLoanSimpleReceiverBase.sol";
import "@aave/core-v3/contracts/interfaces/IPool.sol";
import "@uniswap/v2-periphery/contracts/interfaces/IUniswapV2Router02.sol";
//import "@uniswap/v3-periphery/contracts/interfaces/ISwapRouter.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";

interface ISwapRouter {
    struct ExactInputSingleParams {
        address tokenIn;
        address tokenOut;
        uint24 fee;
        address recipient;
        uint256 amountIn;
        uint256 amountOutMinimum;
        uint160 sqrtPriceLimitX96;
    }

    /// @notice Swaps `amountIn` of one token for as much as possible of another token
    /// @dev Setting `amountIn` to 0 will cause the contract to look up its own balance,
    /// and swap the entire amount, enabling contracts to send tokens before calling this function.
    /// @param params The parameters necessary for the swap, encoded as `ExactInputSingleParams` in calldata
    /// @return amountOut The amount of the received token
    function exactInputSingle(ExactInputSingleParams calldata params) external payable returns (uint256 amountOut);
}

interface V3Deadline {
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

    /// @notice Swaps `amountIn` of one token for as much as possible of another token
    /// @param params The parameters necessary for the swap, encoded as `ExactInputSingleParams` in calldata
    /// @return amountOut The amount of the received token
    function exactInputSingle(ExactInputSingleParams memory params) external payable returns (uint256 amountOut);
}

contract FlashSwap is FlashLoanSimpleReceiverBase {
    struct SwapStep {
        address poolAddress;
        address tokenIn;
        address tokenOut;
        uint8 protocol;
        uint24 fee;  // For V3 swaps
    }

    uint160 internal constant MIN_SQRT_RATIO = 4295128739;
    uint160 internal constant MAX_SQRT_RATIO = 1461446703485210103287273052203988822378723970342;

    // Default uniswap routers, these are all the same
    address constant UNISWAP_V2_ROUTER = 0x4752ba5DBc23f44D87826276BF6Fd6b1C372aD24; // swap exact tokens for tokens 
    address constant SUSHISWAP_V2_ROUTER = 0x6BDED42c6DA8FBf0d2bA55B2fa120C5e0c8D7891; // swap exact tokens for tokens
    address constant PANCAKESWAP_V2_ROUTER = 0x8cFe327CEc66d1C090Dd72bd0FF11d690C33a2Eb; // swap exact tokens for tokens
    address constant BASESWAP_V2_ROUTER = 0x327Df1E6de05895d2ab08513aaDD9313Fe505d86; // swap exact tokens for tokens

    // Default uniswapv3 router
    address constant UNISWAP_V3_ROUTER = 0x2626664c2603336E57B271c5C0b26F421741e481; // exactInputSingle
    address constant PANCAKESWAP_V3_ROUTER = 0x678Aa4bF4E210cf2166753e054d5b7c31cc7fa86; // exactInputSingle

    // no swapExactTokensFortokens, have exactInput
    address constant SUSHISWAP_V3_ROUTER = 0xFB7eF66a7e61224DD6FcD0D7d9C3be5C8B049b9f; // exactInputSingke, but has the deadline param
    address constant BASESWAP_V3_ROUTER = 0x1B8eea9315bE495187D873DA7773a874545D9D48; // exactInputSingle, but has the deadline param
    address constant SLIPSTREAM_ROUTER = 0xBE6D8f0d05cC4be24d5167a3eF062215bE6D18a5; // exactInputSingle, but has the deadline param


    // this is swap exact tokens for tokens but has a different route, tuple instead of array, .. annoying
    address constant AERODOME_ROUTER = 0xcF77a3Ba9A5CA399B7c97c74d54e5b1Beb874E43; // swapExactTokensFortokens, tuple





    address constant AAVE_ADDRESSES_PROVIDER = 0xe20fCBdBfFC4Dd138cE8b2E6FBb6CB49777ad64D;

    address public immutable owner;

    event Profit(uint256 value);

    constructor() FlashLoanSimpleReceiverBase(IPoolAddressesProvider(AAVE_ADDRESSES_PROVIDER)) {
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

        (SwapStep[] memory steps, address caller) = abi.decode(params, (SwapStep[], address));

        uint256 amountIn = amount;

        for (uint i = 0; i < steps.length; i++) {
            SwapStep memory step = steps[i];
            
            uint256 balanceBefore = IERC20(step.tokenIn).balanceOf(address(this));

            if (step.protocol <= 2) {
                _swapV2(step.tokenIn, step.tokenOut, balanceBefore, _getV2Router(step.protocol));
            } else if (step.protocol <= 5) {
                _swapV3(step.tokenIn, step.tokenOut, balanceBefore, step.fee, _getV3Router(step.protocol));
            } else {
                _swapV3Deadline(step.tokenIn, step.tokenOut, balanceBefore, step.fee, _getV3RouterDeadline(step.protocol));
            }

            uint256 balanceAfter = IERC20(step.tokenOut).balanceOf(address(this));

            amountIn = balanceAfter;
        }

        // Repay the flash loan
        uint256 amountToRepay = amount + premium;
        IERC20(asset).approve(address(POOL), amountToRepay);

        // Calculate and transfer profit
        uint256 finalBalance = IERC20(asset).balanceOf(address(this));
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
            0, // Accept any amount of output tokens
            path,
            address(this),
            block.timestamp
        );
    }

    function _swapV3(address tokenIn, address tokenOut, uint256 amountIn, uint24 fee, address router) internal {
        IERC20(tokenIn).approve(router, amountIn);

        ISwapRouter.ExactInputSingleParams memory params = ISwapRouter.ExactInputSingleParams({
            tokenIn: tokenIn,
            tokenOut: tokenOut,
            fee: fee,
            recipient: address(this),
            amountIn: amountIn,
            amountOutMinimum: 0,
            sqrtPriceLimitX96: 0
        });

        ISwapRouter(router).exactInputSingle(params);
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

    function _getV3RouterAerodome(uint8 protocol) internal pure returns (address) {
        if (protocol == 9) return AERODOME_ROUTER;
        revert("Invalid V3 protocol");
    }


    function withdraw(address token) external {
        require(msg.sender == owner, "Only owner can withdraw");
        uint256 balance = IERC20(token).balanceOf(address(this));
        IERC20(token).transfer(owner, balance);
    }
}