// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@uniswap/v2-core/contracts/interfaces/IUniswapV2Pair.sol";
import "@uniswap/v2-core/contracts/interfaces/IUniswapV2Callee.sol";
import "@uniswap/v2-core/contracts/interfaces/IUniswapV2Factory.sol";
import "@uniswap/v2-periphery/contracts/interfaces/IUniswapV2Router02.sol";
import "forge-std/console.sol";

contract FlashSwapper is IUniswapV2Callee {
    event Log(string message, uint val);

    address public constant WETH = 0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2;
    address public constant DAI = 0x6B175474E89094C44Da98b954EedeAC495271d0F;
    address public constant UNISWAP_V2_FACTORY = 0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f;

    IERC20 private constant weth = IERC20(WETH);
    IUniswapV2Factory private constant factory = IUniswapV2Factory(UNISWAP_V2_FACTORY);
    IUniswapV2Pair public immutable pair;

    error FlashSwapError(
        address msgSender,
        address caller,
        address pair,
        address tokenBorrow,
        uint256 amount0,
        uint256 amount1,
        uint256 fee,
        uint256 amountToRepay,
        uint256 callerBalance,
        uint256 callerAllowance
    );

    constructor() {
        pair = IUniswapV2Pair(factory.getPair(DAI, WETH));
        console.log("Constructor: pair address", address(pair));
    }

    function flashSwap(uint wethAmount) external {
        console.log("flashSwap called by", msg.sender);
        console.log("WETH amount requested", wethAmount);
        
        require(wethAmount > 0, "FlashSwap: amount must be greater than 0");
        require(pair != IUniswapV2Pair(address(0)), "FlashSwap: pair not initialized");
        
        console.log("FlashSwap: pair address", address(pair));
        console.log("FlashSwap: this contract address", address(this));
        require(msg.sender != address(0), "FlashSwap: msg.sender is zero address");
        
        bytes memory data = abi.encode(WETH, msg.sender);
        console.log("FlashSwap: encoded data length", data.length);
        
        console.log("FlashSwap: calling pair.swap");
        pair.swap(0, wethAmount, address(this), data);
        console.log("FlashSwap: pair.swap completed");
    }

    function uniswapV2Call(
        address sender,
        uint amount0,
        uint amount1,
        bytes calldata data
    ) external override {
        console.log("uniswapV2Call entered");
        console.log("uniswapV2Call msg.sender", msg.sender);
        console.log("uniswapV2Call sender", sender);
        console.log("uniswapV2Call amount0", amount0);
        console.log("uniswapV2Call amount1", amount1);
        console.log("uniswapV2Call data length", data.length);

        (address tokenBorrow, address caller) = abi.decode(data, (address, address));
        console.log("uniswapV2Call tokenBorrow", tokenBorrow);
        console.log("uniswapV2Call caller", caller);

        uint fee = ((amount1 * 3) / 997) + 1;
        uint amountToRepay = amount1 + fee;
        console.log("uniswapV2Call fee", fee);
        console.log("uniswapV2Call amountToRepay", amountToRepay);

        uint callerBalance = IERC20(WETH).balanceOf(caller);
        uint callerAllowance = IERC20(WETH).allowance(caller, address(this));
        console.log("uniswapV2Call callerBalance", callerBalance);
        console.log("uniswapV2Call callerAllowance", callerAllowance);

        console.log("uniswapV2Call this contract's WETH balance", IERC20(WETH).balanceOf(address(this)));

        console.log("uniswapV2Call checking conditions");
        if (msg.sender != address(pair)) {
            console.log("uniswapV2Call condition failed: msg.sender != address(pair)");
        }
        if (sender != address(this)) {
            console.log("uniswapV2Call condition failed: sender != address(this)");
        }
        if (tokenBorrow != WETH) {
            console.log("uniswapV2Call condition failed: tokenBorrow != WETH");
        }
        if (callerBalance < amountToRepay) {
            console.log("uniswapV2Call condition failed: callerBalance < amountToRepay");
        }
        if (callerAllowance < amountToRepay) {
            console.log("uniswapV2Call condition failed: callerAllowance < amountToRepay");
        }

        if (msg.sender != address(pair) ||
            sender != address(this) ||
            tokenBorrow != WETH ||
            callerBalance < amountToRepay ||
            callerAllowance < amountToRepay) {
            console.log("uniswapV2Call reverting with FlashSwapError");
            revert FlashSwapError(
                msg.sender,
                caller,
                address(pair),
                tokenBorrow,
                amount0,
                amount1,
                fee,
                amountToRepay,
                callerBalance,
                callerAllowance
            );
        }

        console.log("uniswapV2Call performing transferFrom");
        require(IERC20(WETH).transferFrom(caller, address(this), amountToRepay), "FlashSwap: transferFrom failed");
        
        console.log("uniswapV2Call performing transfer to pair");
        require(IERC20(WETH).transfer(address(pair), amountToRepay), "FlashSwap: transfer to pair failed");

        console.log("uniswapV2Call completed successfully");
    }

    function check_allowance(address owner) public view returns (uint256) {
        return IERC20(WETH).allowance(owner, address(this));
    }
}