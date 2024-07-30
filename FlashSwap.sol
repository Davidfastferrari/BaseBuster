// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import "@uniswap/v2-core/contracts/interfaces/IUniswapV2Pair.sol";
import "@uniswap/v2-periphery/contracts/interfaces/IUniswapV2Router02.sol";
import "@uniswap/v3-core/contracts/interfaces/IUniswapV3Pool.sol";
import "@uniswap/v3-periphery/contracts/interfaces/ISwapRouter.sol";

contract MultiProtocolArbitrage {
    using SafeERC20 for IERC20;

    address public owner;

    enum Protocol { UniswapV2, UniswapV3, Sushiswap }

    struct SwapStep {
        Protocol protocol;
        address tokenIn;
        address tokenOut;
        uint24 fee; // Used for Uniswap V3, 0 for others
        address pool; // Used for V2 protocols, address(0) for V3
    }

    struct FlashLoanParams {
        address tokenBorrow;
        uint256 amountBorrow;
        SwapStep[] path;
    }

    // Protocol-specific router addresses
    address constant UNISWAP_V2_ROUTER = 0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D;
    address constant UNISWAP_V3_ROUTER = 0xE592427A0AEce92De3Edee1F18E0157C05861564;
    address constant SUSHISWAP_ROUTER = 0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F;

    constructor() {
        owner = msg.sender;
    }

    function executeArbitrage(SwapStep[] calldata _path, uint256 _amountBorrow) external {
        require(_path.length >= 2, "Path too short");
        require(msg.sender == owner, "Only owner");

        FlashLoanParams memory params = FlashLoanParams({
            tokenBorrow: _path[0].tokenIn,
            amountBorrow: _amountBorrow,
            path: _path
        });

        if (_path[0].protocol == Protocol.UniswapV3) {
            IUniswapV3Pool(_path[0].pool).flash(
                address(this),
                _amountBorrow,
                0,
                abi.encode(params)
            );
        } else {
            // UniswapV2 or Sushiswap
            IUniswapV2Pair pair = IUniswapV2Pair(_path[0].pool);
            pair.swap(
                _path[0].tokenIn < _path[0].tokenOut ? 0 : _amountBorrow,
                _path[0].tokenIn < _path[0].tokenOut ? _amountBorrow : 0,
                address(this),
                abi.encode(params)
            );
        }
    }

    function uniswapV3FlashCallback(uint256 fee0, uint256 fee1, bytes calldata data) external {
        FlashLoanParams memory params = abi.decode(data, (FlashLoanParams));
        require(msg.sender == params.path[0].pool, "Unauthorized");

        uint256 amountOwed = params.amountBorrow + fee0 + fee1;
        _executeArbitrage(params, amountOwed);
    }

    function uniswapV2Call(address sender, uint256 amount0, uint256 amount1, bytes calldata data) external {
        require(sender == address(this), "Unauthorized");
        FlashLoanParams memory params = abi.decode(data, (FlashLoanParams));
        require(msg.sender == params.path[0].pool, "Unauthorized");

        uint256 amountBorrowed = amount0 > 0 ? amount0 : amount1;
        uint256 amountOwed = (amountBorrowed * 1000) / 997 + 1; // Account for 0.3% fee
        _executeArbitrage(params, amountOwed);
    }

    function _executeArbitrage(FlashLoanParams memory params, uint256 amountToRepay) private {
        uint256 currentAmount = params.amountBorrow;

        for (uint i = 1; i < params.path.length; i++) {
            currentAmount = _swap(params.path[i], currentAmount);
        }

        require(currentAmount > amountToRepay, "Arbitrage not profitable");

        // Repay flash loan
        IERC20(params.path[params.path.length - 1].tokenOut).safeTransfer(params.path[0].pool, amountToRepay);

        // Transfer profit to owner
        uint256 profit = currentAmount - amountToRepay;
        IERC20(params.path[params.path.length - 1].tokenOut).safeTransfer(owner, profit);
    }

    function _swap(SwapStep memory step, uint256 amountIn) private returns (uint256) {
        if (step.protocol == Protocol.UniswapV3) {
            return _swapUniswapV3(step, amountIn);
        } else {
            return _swapUniswapV2(step, amountIn);
        }
    }

    function _swapUniswapV3(SwapStep memory step, uint256 amountIn) private returns (uint256) {
        ISwapRouter.ExactInputSingleParams memory params = ISwapRouter.ExactInputSingleParams({
            tokenIn: step.tokenIn,
            tokenOut: step.tokenOut,
            fee: step.fee,
            recipient: address(this),
            deadline: block.timestamp,
            amountIn: amountIn,
            amountOutMinimum: 0,
            sqrtPriceLimitX96: 0
        });

        return ISwapRouter(UNISWAP_V3_ROUTER).exactInputSingle(params);
    }

    function _swapUniswapV2(SwapStep memory step, uint256 amountIn) private returns (uint256) {
        address[] memory path = new address[](2);
        path[0] = step.tokenIn;
        path[1] = step.tokenOut;

        address router = step.protocol == Protocol.UniswapV2 ? UNISWAP_V2_ROUTER : SUSHISWAP_ROUTER;

        IERC20(step.tokenIn).safeApprove(router, amountIn);

        uint256[] memory amounts = IUniswapV2Router02(router).swapExactTokensForTokens(
            amountIn,
            0, // Accept any amount
            path,
            address(this),
            block.timestamp
        );

        return amounts[1];
    }

    function withdrawToken(address token) external {
        require(msg.sender == owner, "Only owner");
        IERC20(token).safeTransfer(owner, IERC20(token).balanceOf(address(this)));
    }

    function withdrawETH() external {
        require(msg.sender == owner, "Only owner");
        payable(owner).transfer(address(this).balance);
    }

    receive() external payable {}
}