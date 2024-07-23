// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import "forge-std/Test.sol";
import "../src/Arb.sol";
import "@uniswap/v2-core/contracts/interfaces/IUniswapV2Pair.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";

contract FlashSwapperTest is Test {
    FlashSwapper public flashSwapper;
    address public constant WETH = 0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2;
    address public constant DAI = 0x6B175474E89094C44Da98b954EedeAC495271d0F;
    address public constant USDC = 0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48;

    IUniswapV2Pair pairWETH_DAI;
    IUniswapV2Pair pairDAI_USDC;
    IUniswapV2Pair pairUSDC_WETH;

    function setUp() public {
        vm.createSelectFork("mainnet", 20370439);
        flashSwapper = new FlashSwapper();

        // Get Uniswap V2 pairs
        pairWETH_DAI = IUniswapV2Pair(flashSwapper.factory().getPair(WETH, DAI));
        pairDAI_USDC = IUniswapV2Pair(flashSwapper.factory().getPair(DAI, USDC));
        pairUSDC_WETH = IUniswapV2Pair(flashSwapper.factory().getPair(USDC, WETH));

        // Provide initial balances to FlashSwapper
        deal(WETH, address(flashSwapper), 10 ether);
        deal(DAI, address(flashSwapper), 10000 ether);
        deal(USDC, address(flashSwapper), 10000 * 10**6);

        // Ensure the FlashSwapper contract has some ETH for gas
        vm.deal(address(flashSwapper), 1 ether);
    }

    function testFlashSwapArbitrage() public {
        // Manipulate reserves to create arbitrage opportunity
        manipulateReserves();

        // Record initial WETH balance
        uint256 initialWethBalance = IERC20(WETH).balanceOf(address(flashSwapper));

        // Set up flash swap path
        address[] memory path = new address[](4);
        path[0] = WETH;
        path[1] = DAI;
        path[2] = USDC;
        path[3] = WETH;

        // Execute flash swap with a smaller amount
        flashSwapper.flashSwap(0.1 ether, path);

        // Check final WETH balance
        uint256 finalWethBalance = IERC20(WETH).balanceOf(address(flashSwapper));

        console.log("Initial WETH balance:", initialWethBalance);
        console.log("Final WETH balance:", finalWethBalance);

        assertTrue(finalWethBalance > initialWethBalance, "No profit made");
        console.log("Profit in WETH:", finalWethBalance - initialWethBalance);
    }

    function manipulateReserves() internal {
        // Manipulate WETH-DAI pair
        vm.prank(address(pairWETH_DAI));
        IERC20(WETH).transfer(address(this), 100 ether);
        pairWETH_DAI.sync();

        // Manipulate DAI-USDC pair
        vm.prank(address(pairDAI_USDC));
        IERC20(DAI).transfer(address(this), 100000 ether);
        pairDAI_USDC.sync();

        // Manipulate USDC-WETH pair
        vm.prank(address(pairUSDC_WETH));
        IERC20(USDC).transfer(address(this), 100000 * 10**6);
        pairUSDC_WETH.sync();
    }
}