// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import "@aave/core-v3/contracts/flashloan/base/FlashLoanSimpleReceiverBase.sol";
import "@aave/core-v3/contracts/interfaces/IPool.sol";
import "@uniswap/v2-periphery/contracts/interfaces/IUniswapV2Router02.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";

contract FlashSwap is FlashLoanSimpleReceiverBase {
    struct SwapStep {
        address poolAddress;
        address tokenIn;
        address tokenOut;
        uint8 protocol;
    }

    // Hardcoded addresses
    address constant UNISWAP_V2_ROUTER = 0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D;
    address constant SUSHISWAP_ROUTER = 0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F;
    address constant PANCAKESWAP_ROUTER = 0x10ED43C718714eb63d5aA57B78B54704E256024E;
    address constant AAVE_ADDRESSES_PROVIDER = 0x2f39d218133AFaB8F2B819B1066c7E434Ad94E9e;

    address public owner;

    event DebugLog(string message, uint256 value);
    event Profit(uint256 value);
    event DebugAddress(string message, address value);

    constructor() FlashLoanSimpleReceiverBase(IPoolAddressesProvider(AAVE_ADDRESSES_PROVIDER)) {
<<<<<<< HEAD
=======
        owner = msg.sender;
>>>>>>> 86750add12efb7ea2e4a20a99cd4e7b0550e3e74
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

            if (step.protocol == 0) {
                _swap(step.tokenIn, step.tokenOut, balanceBefore, UNISWAP_V2_ROUTER);
            } else if (step.protocol == 1) {
                _swap(step.tokenIn, step.tokenOut, balanceBefore, SUSHISWAP_ROUTER);
            } else if (step.protocol == 2) {
                _swap(step.tokenIn, step.tokenOut, balanceBefore, PANCAKESWAP_ROUTER);
            }

            uint256 balanceAfter = IERC20(step.tokenOut).balanceOf(address(this));
            emit DebugLog("Balance after swap", balanceAfter);

            amountIn = balanceAfter;
        }

        // Repay the flash loan
        uint256 amountToRepay = amount + premium;
        IERC20(asset).approve(address(POOL), amountToRepay);

        emit DebugLog("Amount to repay", amountToRepay);

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

    function _swap(address tokenIn, address tokenOut, uint256 amountIn, address router) internal {
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

    function withdraw(address token) external {
        require(msg.sender == owner, "Only owner can withdraw");
        uint256 balance = IERC20(token).balanceOf(address(this));
        IERC20(token).transfer(owner, balance);
        emit DebugLog("Withdrawn", balance);
    }
<<<<<<< HEAD
}
=======
}
>>>>>>> 86750add12efb7ea2e4a20a99cd4e7b0550e3e74
