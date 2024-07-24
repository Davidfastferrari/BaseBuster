// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import "@uniswap/v2-core/contracts/interfaces/IUniswapV2Pair.sol";
import "@uniswap/v2-core/contracts/interfaces/IUniswapV2Factory.sol";
import "@uniswap/v2-periphery/contracts/interfaces/IUniswapV2Router02.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";

/**
 * @title Swap
 * @dev A contract for performing token swaps using Uniswap V2
 */
contract Swap {
    using SafeERC20 for IERC20;
    IUniswapV2Router02 public immutable router;

    // Custom errors
    error InvalidPath();
    error InsufficientAllowance(uint256 required, uint256 allowed);
    error InsufficientBalance(uint256 required, uint256 balance);
    error SwapFailed(uint256 amountIn, uint256 amountOutMin);
    error DeadlineExceeded(uint256 deadline, uint256 currentTimestamp);

    /**
     * @dev Constructor that sets the Uniswap V2 Router address
     */
    constructor() {
        router = IUniswapV2Router02(0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D);
    }

    /**
     * @dev Performs a token swap
     * @param amountIn The amount of input tokens
     * @param amountOutMin The minimum amount of output tokens to receive
     * @param path An array of token addresses representing the swap path
     * @param deadline The deadline for the swap to be completed
     * @return amounts An array of amounts for each step in the swap path
     */
    function swap(
        uint256 amountIn,
        uint256 amountOutMin,
        address[] calldata path,
        uint256 deadline
    ) external returns (uint[] memory amounts) {
        if (path.length < 2) revert InvalidPath();
        if (deadline < block.timestamp) revert DeadlineExceeded(deadline, block.timestamp);

        IERC20 tokenIn = IERC20(path[0]);
        
        // Check allowance
        uint256 allowance = tokenIn.allowance(msg.sender, address(this));
        if (allowance < amountIn) revert InsufficientAllowance(amountIn, allowance);

        // Check balance
        uint256 balance = tokenIn.balanceOf(msg.sender);
        if (balance < amountIn) revert InsufficientBalance(amountIn, balance);

        // Transfer tokens from the sender to this contract
        tokenIn.safeTransferFrom(msg.sender, address(this), amountIn);
        
        // Approve the router to spend tokens
        tokenIn.approve(address(router), amountIn);

        // Perform the swap
        try router.swapExactTokensForTokens(
            amountIn,
            amountOutMin,
            path,
            msg.sender, // Tokens will be sent directly to the sender
            deadline
        ) returns (uint[] memory _amounts) {
            amounts = _amounts;
        } catch {
            revert SwapFailed(amountIn, amountOutMin);
        }

        // If there are any tokens left in the contract, send them back to the sender
        uint256 leftover = tokenIn.balanceOf(address(this));
        if (leftover > 0) {
            tokenIn.safeTransfer(msg.sender, leftover);
        }

        return amounts;
    }


    function getOut(uint256 _amountIn, address[] calldata _path)
}