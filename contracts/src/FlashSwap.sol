// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import "@uniswap/v2-core/contracts/interfaces/IUniswapV2Factory.sol";
import "@uniswap/v2-core/contracts/interfaces/IUniswapV2Pair.sol";
import "@uniswap/v2-periphery/contracts/interfaces/IUniswapV2Router02.sol";
import "@uniswap/v3-core/contracts/interfaces/IUniswapV3Factory.sol";
import "@uniswap/v3-core/contracts/interfaces/IUniswapV3Pool.sol";
import "@uniswap/v3-periphery/contracts/interfaces/ISwapRouter.sol";
import "@uniswap/v3-core/contracts/interfaces/callback/IUniswapV3FlashCallback.sol";



contract FlashSwap {
    using SafeERC20 for IERC20;

    // the interfaces for the dexs
    IUniswapV2Router02 public immutable uniswapV2Router;
    IUniswapV2Router02 public immutable sushiswapRouter;
    ISwapRouter public immutable uniswapV3Router;

    IUniswapV2Factory public immutable uniswapV2Factory;
    IUniswapV2Factory public immutable sushiswapFactory;
    IUniswapV3Factory public immutable uniswapV3Factory;


    address public immutable WETH;

    enum DEX {
        UniswapV2,
        UniswapV3,
        Sushiswap
    }

    struct SwapStep {
        address tokenIn;
        address tokenOut;
        uint24 fee;
        DEX dex;
    }

    struct FlashParams {
        address token0;
        address token1;
        uint24 fee;
        uint256 amount0;
        uint256 amount1;
        SwapStep[] path;
    }

    constructor() {
        uniswapV2Router = IUniswapV2Router02(0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D);
        uniswapV3Router = ISwapRouter(0xE592427A0AEce92De3Edee1F18E0157C05861564);
        sushiswapRouter = IUniswapV2Router02(0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F);
        uniswapV2Factory = IUniswapV2Factory(0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f);
        sushiswapFactory = IUniswapV2Factory(0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac);
        uniswapV3Factory = IUniswapV3Factory(0x1F98431c8aD98523631AE4a59f267346ea31F984);
        WETH = 0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2;
    }

    // Main function to execute the flash swap
    function executeFlashSwap(
        uint256 amount,
        SwapStep[] calldata path
    ) external {
        require(path.length >= 2, "Path too short");
        require(path[0].tokenIn == WETH && path[path.length - 1].tokenOut == WETH, "Path must start and end with WETH");

        if (path[0].dex == DEX.UniswapV3) {
            // For Uniswap V3, we use the flash function
            IUniswapV3Pool pool = IUniswapV3Pool(
                uniswapV3Factory.getPool(path[0].tokenIn, path[0].tokenOut, path[0].fee)
            );
            require(address(pool) != address(0), "V3 pool not found");

            pool.flash(
                address(this),
                amount,
                0,
                abi.encode(FlashParams({
                    token0: path[0].tokenIn,
                    token1: path[0].tokenOut,
                    fee: path[0].fee,
                    amount0: amount,
                    amount1: 0,
                    path: path
                }))
            );
        } else {
            // For Uniswap V2 or SushiSwap, we use the swap function
            IUniswapV2Factory factory = (path[0].dex == DEX.UniswapV2) ? uniswapV2Factory : sushiswapFactory;
            address pairAddress = factory.getPair(path[0].tokenIn, path[0].tokenOut);
            require(pairAddress != address(0), "V2 pair not found");
            IUniswapV2Pair pair = IUniswapV2Pair(pairAddress);

            (uint256 amount0Out, uint256 amount1Out) = path[0].tokenIn < path[0].tokenOut
                ? (uint256(0), amount)
                : (amount, uint256(0));

            pair.swap(
                amount0Out,
                amount1Out,
                address(this),
                abi.encode(FlashParams({
                    token0: path[0].tokenIn,
                    token1: path[0].tokenOut,
                    fee: 0,
                    amount0: amount0Out,
                    amount1: amount1Out,
                    path: path
                }))
            );
        }
    }

    // This function is called by the Uniswap V3 pool for flash loans
    function uniswapV3FlashCallback(
        uint256 fee0,
        uint256 fee1,
        bytes calldata data
    ) external {
        FlashParams memory params = abi.decode(data, (FlashParams));
        require(msg.sender == uniswapV3Factory.getPool(params.token0, params.token1, params.fee), "Unauthorized");

        executeArbitrage(params);

        // Repay the flash loan
        uint256 amount0Owed = params.amount0 + fee0;
        uint256 amount1Owed = params.amount1 + fee1;
        if (amount0Owed > 0) IERC20(params.token0).safeTransfer(msg.sender, amount0Owed);
        if (amount1Owed > 0) IERC20(params.token1).safeTransfer(msg.sender, amount1Owed);
    }

    // This function is called by the Uniswap V2 pair contract
    function uniswapV2Call(
        address sender,
        uint256 amount0,
        uint256 amount1,
        bytes calldata data
    ) external {
        FlashParams memory params = abi.decode(data, (FlashParams));
        IUniswapV2Factory factory = (params.path[0].dex == DEX.UniswapV2) ? uniswapV2Factory : sushiswapFactory;
        require(msg.sender == factory.getPair(params.token0, params.token1), "Unauthorized");
        require(sender == address(this), "Unauthorized");

        executeArbitrage(params);

        // Repay the flash loan
        uint256 amountOwed = ((amount0 > 0) ? amount0 : amount1) * 1000 / 997 + 1;
        IERC20(params.path[0].tokenOut).safeTransfer(msg.sender, amountOwed);
    }

    // Helper function to execute the arbitrage
    function executeArbitrage(FlashParams memory params) private {
        uint256 currentAmount = (params.amount0 > 0) ? params.amount0 : params.amount1;

        for (uint i = 1; i < params.path.length; i++) {
            SwapStep memory step = params.path[i];
            currentAmount = executeSwap(step, currentAmount);
        }

        // Ensure we have enough to repay the flash loan
        require(currentAmount > params.amount0 + params.amount1, "Arbitrage not profitable");

        // Transfer the profit to the contract
        IERC20(WETH).safeTransfer(address(this), currentAmount - (params.amount0 + params.amount1));
    }

    // Helper function to execute a single swap step
    function executeSwap(SwapStep memory step, uint256 amountIn) private returns (uint256) {
        address router = getRouterForDEX(step.dex);
        
        // Approve the router to spend the input token
        IERC20(step.tokenIn).approve(router, 0); // First, clear any existing allowance
        IERC20(step.tokenIn).approve(router, amountIn);

        if (step.dex == DEX.UniswapV3) {
            return swapOnUniswapV3(step, amountIn);
        } else {
            return swapOnUniswapV2(step, amountIn);
        }
    }

    // Helper function to swap on Uniswap V3
    function swapOnUniswapV3(SwapStep memory step, uint256 amountIn) private returns (uint256) {
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

        return uniswapV3Router.exactInputSingle(params);
    }

    // Helper function to swap on Uniswap V2 or SushiSwap
    function swapOnUniswapV2(SwapStep memory step, uint256 amountIn) private returns (uint256) {
        address[] memory path = new address[](2);
        path[0] = step.tokenIn;
        path[1] = step.tokenOut;

        IUniswapV2Router02 router = (step.dex == DEX.UniswapV2) ? uniswapV2Router : sushiswapRouter;
        uint256[] memory amounts = router.swapExactTokensForTokens(
            amountIn,
            0,
            path,
            address(this),
            block.timestamp
        );

        return amounts[amounts.length - 1];
    }

    // Helper function to get the router for a given DEX
    function getRouterForDEX(DEX dex) private view returns (address) {
        if (dex == DEX.UniswapV2) {
            return address(uniswapV2Router);
        } else if (dex == DEX.Sushiswap) {
            return address(sushiswapRouter);
        } else {
            return address(uniswapV3Router);
        }
    }

    // Function to withdraw tokens from the contract
    function withdrawToken(address token, uint256 amount) external {
        IERC20(token).safeTransfer(msg.sender, amount);
    }

    // Function to withdraw ETH from the contract
    function withdrawETH(uint256 amount) external {
        payable(msg.sender).transfer(amount);
    }

    // Allow the contract to receive ETH
    receive() external payable {} 

}


























