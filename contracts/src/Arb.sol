// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import "@uniswap/v2-core/contracts/interfaces/IUniswapV2Pair.sol";
import "@uniswap/v2-core/contracts/interfaces/IUniswapV2Factory.sol";
import "@uniswap/v2-periphery/contracts/interfaces/IUniswapV2Router02.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "forge-std/console.sol";

contract FlashSwapper {
    IUniswapV2Factory public immutable factory;
    IUniswapV2Router02 public immutable router;
    address public constant WETH = 0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2;

    constructor() {
        factory = IUniswapV2Factory(0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f);
        router = IUniswapV2Router02(0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D);
    }

    function flashSwap(uint256 amountIn, address[] calldata path) external {
        console.log("Initiating flash swap with amount", amountIn);
        console.log("First token:", path[0]);
        console.log("Last token:", path[path.length - 1]);
        console.log("Sender:", msg.sender);

        address pair = factory.getPair(path[0], path[1]);  // Fixed this line
        bytes memory data = abi.encode(msg.sender, amountIn, path);
        uint256 amount0Out = path[0] < path[1] ? amountIn : 0;
        uint256 amount1Out = path[0] < path[1] ? 0 : amountIn;

        IUniswapV2Pair(pair).swap(amount0Out, amount1Out, address(this), data);
    }

    // This function needs to be implemented to handle the flash swap callback
    function uniswapV2Call(
        address sender,
        uint256 amount0,
        uint256 amount1,
        bytes calldata data
    ) external {
        // Decode the flash swap data
        (address initiator, uint256 amountIn, address[] memory path) = abi
            .decode(data, (address, uint256, address[]));

        // Verify the caller is a valid pair
        address token0 = IUniswapV2Pair(msg.sender).token0();
        address token1 = IUniswapV2Pair(msg.sender).token1();
        require(
            msg.sender == IUniswapV2Factory(factory).getPair(token0, token1),
            "Unauthorized"
        );

        // Calculate the amount borrowed
        uint256 amountBorrowed = amount0 > 0 ? amount0 : amount1;

        // Execute swaps along the path
        for (uint i = 1; i < path.length - 1; i++) {
            (address input, address output) = (path[i], path[i + 1]);
            (uint256 reserveInput, uint256 reserveOutput, ) = IUniswapV2Pair(
                IUniswapV2Factory(factory).getPair(input, output)
            ).getReserves();
            console.log("Input: ", input);
            console.log("Output: ", output);
            console.log("Reserve Input: ", reserveInput);
            console.log("Reserve Output: ", reserveOutput);
            uint256 amountOut = getAmountOut(
                amountBorrowed,
                reserveInput,
                reserveOutput
            );
            console.log("Amount Out: ", amountOut);
            _swap(amountBorrowed, amountOut, path[i], path[i + 1]);
            amountBorrowed = amountOut;
        }

        // Calculate the amount to repay
        (uint256 reserveIn, uint256 reserveOut, ) = IUniswapV2Pair(msg.sender)
            .getReserves();
        uint256 amountToRepay = getAmountIn(
            amountBorrowed,
            reserveIn,
            reserveOut
        );

        // Repay the flash swap
        IERC20(path[path.length - 1]).transfer(msg.sender, amountToRepay);

        // Transfer any profit to the initiator
        uint256 profit = IERC20(path[path.length - 1]).balanceOf(address(this));
        if (profit > 0) {
            IERC20(path[path.length - 1]).transfer(initiator, profit);
        }
    }

    // Helper function to calculate amount out based on xy=k formula
    function getAmountOut(
        uint256 amountIn,
        uint256 reserveIn,
        uint256 reserveOut
    ) internal pure returns (uint256) {
        uint256 amountInWithFee = amountIn * 997;
        uint256 numerator = amountInWithFee * reserveOut;
        uint256 denominator = reserveIn * 1000 + amountInWithFee;
        return numerator / denominator;
    }

    // Helper function to calculate amount in based on xy=k formula
    function getAmountIn(
        uint256 amountOut,
        uint256 reserveIn,
        uint256 reserveOut
    ) internal pure returns (uint256) {
        uint256 numerator = reserveIn * amountOut * 1000;
        uint256 denominator = (reserveOut - amountOut) * 997;
        return (numerator / denominator) + 1;
    }

    // Helper function to execute a single swap
    function _swap(
        uint256 amountIn,
        uint256 amountOut,
        address tokenIn,
        address tokenOut
    ) internal {
        address pair = IUniswapV2Factory(factory).getPair(tokenIn, tokenOut);
        (uint256 amount0Out, uint256 amount1Out) = tokenIn < tokenOut
            ? (uint256(0), amountOut)
            : (amountOut, uint256(0));
        IERC20(tokenIn).transfer(pair, amountIn);
        IUniswapV2Pair(pair).swap(
            amount0Out,
            amount1Out,
            address(this),
            new bytes(0)
        );
    }
}
