// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import "@aave/core-v3/contracts/flashloan/base/FlashLoanSimpleReceiverBase.sol";
import "@aave/core-v3/contracts/interfaces/IPool.sol";
import "@uniswap/v2-periphery/contracts/interfaces/IUniswapV2Router02.sol";
import "@uniswap/v3-periphery/contracts/interfaces/ISwapRouter.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";

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

    // Hardcoded addresses
    address constant UNISWAP_V2_ROUTER = 0x4752ba5DBc23f44D87826276BF6Fd6b1C372aD24;
    address constant SUSHISWAP_ROUTER = 0x6BDED42c6DA8FBf0d2bA55B2fa120C5e0c8D7891;
    address constant PANCAKESWAP_ROUTER = 0x8cFe327CEc66d1C090Dd72bd0FF11d690C33a2Eb;
    address constant UNISWAP_V3_ROUTER = 0x2626664c2603336E57B271c5C0b26F421741e481;
    address constant SUSHISWAP_V3_ROUTER = 0xFB7eF66a7e61224DD6FcD0D7d9C3be5C8B049b9f; // Example address, replace with actual
    address constant AAVE_ADDRESSES_PROVIDER = 0xe20fCBdBfFC4Dd138cE8b2E6FBb6CB49777ad64D;

    address public owner;

    event DebugLog(string message, uint256 value);
    event Profit(uint256 value);
    event DebugAddress(string message, address value);
    event ActualValue(uint256 value);

    constructor() FlashLoanSimpleReceiverBase(IPoolAddressesProvider(AAVE_ADDRESSES_PROVIDER)) {
        owner = msg.sender;
    }

    function executeArbitrage(SwapStep[] calldata steps, uint256 amount) external {
        require(steps.length > 0, "No swap steps provided");
        require(amount > 0, "Amount must be greater than 0");

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
        emit DebugLog("Flash loan received", amountIn);
        emit DebugAddress("Asset received", asset);

        for (uint i = 0; i < steps.length; i++) {
            SwapStep memory step = steps[i];
            
            emit DebugLog("Step", i);
            emit DebugAddress("Pool Address", step.poolAddress);
            emit DebugAddress("TokenIn", step.tokenIn);
            emit DebugAddress("TokenOut", step.tokenOut);
            
            uint256 balanceBefore = IERC20(step.tokenIn).balanceOf(address(this));
            emit DebugLog("Balance before swap", balanceBefore);

            if (step.protocol <= 2) {
                _swapV2(step.tokenIn, step.tokenOut, balanceBefore, _getV2Router(step.protocol));
            } else {
                _swapV3(step.tokenIn, step.tokenOut, balanceBefore, step.fee, _getV3Router(step.protocol));
            }

            uint256 balanceAfter = IERC20(step.tokenOut).balanceOf(address(this));
            emit DebugLog("Balance after swap", balanceAfter);

            amountIn = balanceAfter;
        }

        // Repay the flash loan
        uint256 amountToRepay = amount + premium;
        IERC20(asset).approve(address(POOL), amountToRepay);

        emit DebugLog("Amount to repay", amountToRepay);
        emit ActualValue(amount);

        // Calculate and transfer profit
        uint256 finalBalance = IERC20(asset).balanceOf(address(this));

        emit ActualValue(finalBalance);

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
            deadline: block.timestamp,
            amountIn: amountIn,
            amountOutMinimum: 0,
            sqrtPriceLimitX96: 0
        });

        ISwapRouter(router).exactInputSingle(params);
    }

    function _getV2Router(uint8 protocol) internal pure returns (address) {
        if (protocol == 0) return UNISWAP_V2_ROUTER;
        if (protocol == 1) return SUSHISWAP_ROUTER;
        if (protocol == 2) return PANCAKESWAP_ROUTER;
        revert("Invalid V2 protocol");
    }

    function _getV3Router(uint8 protocol) internal pure returns (address) {
        if (protocol == 3) return UNISWAP_V3_ROUTER;
        if (protocol == 4) return SUSHISWAP_V3_ROUTER;
        revert("Invalid V3 protocol");
    }

    function withdraw(address token) external {
        require(msg.sender == owner, "Only owner can withdraw");
        uint256 balance = IERC20(token).balanceOf(address(this));
        IERC20(token).transfer(owner, balance);
        emit DebugLog("Withdrawn", balance);
    }
}