// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import "@uniswap/v2-core/contracts/interfaces/IUniswapV2Pair.sol";
import "@uniswap/v2-core/contracts/interfaces/IUniswapV2Factory.sol";
import "@uniswap/v2-periphery/contracts/interfaces/IUniswapV2Router02.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";

contract UniswapV2FlashSwap {
    using SafeERC20 for IERC20;

    IUniswapV2Factory public immutable factory;
    IUniswapV2Router02 public immutable router;
    
    address[] public path;
    uint256 public amountIn;
    address public initiator;

    event FlashSwapInitiated(address tokenBorrow, uint256 amount, address[] path);
    event ProfitGenerated(uint256 profit);
    event SwapCompleted(uint256 amountOut);

    constructor() {
        factory = IUniswapV2Factory(0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f);
        router = IUniswapV2Router02(0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D); 
    }

    function initiateFlashSwap(
        address _tokenBorrow,
        uint256 _amount,
        address[] calldata _path
    ) external {
        require(_path.length >= 2, "Path must have at least 2 tokens");
        require(_path[0] == _tokenBorrow, "First token in path must be borrowed token");

        amountIn = _amount;
        path = _path;
        initiator = msg.sender;

        address pair = factory.getPair(_tokenBorrow, _path[1]);
        require(pair != address(0), "Pair does not exist");

        address token0 = IUniswapV2Pair(pair).token0();
        address token1 = IUniswapV2Pair(pair).token1();

        uint256 amount0Out = _tokenBorrow == token0 ? _amount : 0;
        uint256 amount1Out = _tokenBorrow == token1 ? _amount : 0;

        bytes memory data = abi.encode(_tokenBorrow, _amount, _path);

        emit FlashSwapInitiated(_tokenBorrow, _amount, _path);

        IUniswapV2Pair(pair).swap(amount0Out, amount1Out, address(this), data);
    }

    function uniswapV2Call(
        address sender,
        uint256 amount0,
        uint256 amount1,
        bytes calldata data
    ) external {
        address token0 = IUniswapV2Pair(msg.sender).token0();
        address token1 = IUniswapV2Pair(msg.sender).token1();
        address pair = factory.getPair(token0, token1);
        require(msg.sender == pair, "Unauthorized");
        require(sender == address(this), "Sender must be this contract");

        (address tokenBorrow, uint256 amountBorrow, address[] memory swapPath) = abi.decode(data, (address, uint256, address[]));

        // Perform the multi-hop swap
        uint256 amountOut = _swap(amountBorrow, swapPath);

        emit SwapCompleted(amountOut);

        // Calculate the amount to repay
        uint256 fee = ((amountBorrow * 3) / 997) + 1;
        uint256 amountToRepay = amountBorrow + fee;

        // Repay the flash swap
        IERC20(tokenBorrow).safeTransfer(msg.sender, amountToRepay);

        // Transfer any profit to the initiator
        if (amountOut > amountToRepay) {
            uint256 profit = amountOut - amountToRepay;
            IERC20(swapPath[swapPath.length - 1]).safeTransfer(initiator, profit);
            emit ProfitGenerated(profit);
        }
    }

    function _swap(uint256 _amountIn, address[] memory _path) internal returns (uint256) {
        require(_path.length >= 2, "Invalid path");
        
        uint256[] memory amounts = router.swapExactTokensForTokens(
            _amountIn,
            0, // Accept any amount of output tokens
            _path,
            address(this),
            block.timestamp
        );

        return amounts[amounts.length - 1];
    }

    // Function to rescue tokens sent to the contract by mistake
    function rescueTokens(address _token, uint256 _amount) external {
        require(msg.sender == initiator, "Only initiator can rescue tokens");
        IERC20(_token).safeTransfer(initiator, _amount);
    }
}