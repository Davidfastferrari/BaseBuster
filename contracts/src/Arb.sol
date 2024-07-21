// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import "@openzeppelin/contracts/token/ERC20/IERC20.sol";

interface IUniswapV2Pair {
    function token0() external view returns (address);

    function token1() external view returns (address);

    function swap(
        uint amount0Out,
        uint amount1Out,
        address to,
        bytes calldata data
    ) external;
}

interface IUniswapV2Callee {
    function uniswapV2Call(
        address sender,
        uint amount0,
        uint amount1,
        bytes calldata data
    ) external;
}

contract UniswapV2ArbitrageContract is IUniswapV2Callee {
    address private immutable owner;
    address private flashBorrowPoolAddress;
    uint256[2] private flashBorrowTokenAmounts;
    uint256 private flashRepayTokenAmount;
    address[] private swapPoolAddresses;
    uint256[2][] private swapPoolAmounts;

    constructor() {
        owner = msg.sender;
    }

    modifier onlyOwner() {
        require(msg.sender == owner, "Not owner");
        _;
    }

    function withdraw(address tokenAddress) external onlyOwner {
        uint256 balance = IERC20(tokenAddress).balanceOf(address(this));
        require(balance > 1, "Insufficient balance");
        IERC20(tokenAddress).transfer(owner, balance - 1);
    }

    function execute(
        address _flashBorrowPoolAddress,
        uint256[2] memory _flashBorrowTokenAmounts,
        uint256 _flashRepayTokenAmount,
        address[] memory _swapPoolAddresses,
        uint256[2][] memory _swapPoolAmounts
    ) external onlyOwner {
        require(
            _swapPoolAddresses.length == _swapPoolAmounts.length,
            "Mismatched array lengths"
        );

        flashBorrowPoolAddress = _flashBorrowPoolAddress;
        flashBorrowTokenAmounts = _flashBorrowTokenAmounts;
        flashRepayTokenAmount = _flashRepayTokenAmount;
        swapPoolAddresses = _swapPoolAddresses;
        swapPoolAmounts = _swapPoolAmounts;

        IUniswapV2Pair(_flashBorrowPoolAddress).swap(
            _flashBorrowTokenAmounts[0],
            _flashBorrowTokenAmounts[1],
            address(this),
            abi.encode("flashloan")
        );

        // Gas optimization: Clear storage
        delete flashBorrowPoolAddress;
        delete flashBorrowTokenAmounts;
        delete flashRepayTokenAmount;
        delete swapPoolAddresses;
        delete swapPoolAmounts;
    }

    function uniswapV2Call(
        address sender,
        uint256 amount0,
        uint256 amount1,
        bytes calldata data
    ) external override {
        require(msg.sender == flashBorrowPoolAddress, "Unauthorized");

        address token0 = IUniswapV2Pair(msg.sender).token0();
        address token1 = IUniswapV2Pair(msg.sender).token1();

        // Transfer borrowed amount to the first swap pool
        if (amount0 > 0) {
            IERC20(token0).transfer(swapPoolAddresses[0], amount0);
        } else {
            IERC20(token1).transfer(swapPoolAddresses[0], amount1);
        }

        // Perform swaps
        for (uint i = 0; i < swapPoolAddresses.length; i++) {
            if (i < swapPoolAddresses.length - 1) {
                IUniswapV2Pair(swapPoolAddresses[i]).swap(
                    swapPoolAmounts[i][0],
                    swapPoolAmounts[i][1],
                    swapPoolAddresses[i + 1],
                    new bytes(0)
                );
            } else {
                IUniswapV2Pair(swapPoolAddresses[i]).swap(
                    swapPoolAmounts[i][0],
                    swapPoolAmounts[i][1],
                    address(this),
                    new bytes(0)
                );
            }
        }

        // Repay flash loan
        if (amount0 > 0) {
            require(
                IERC20(token0).transfer(msg.sender, flashRepayTokenAmount),
                "Repayment failed"
            );
        } else {
            require(
                IERC20(token1).transfer(msg.sender, flashRepayTokenAmount),
                "Repayment failed"
            );
        }
    }

    // Function to rescue tokens in case they get stuck
    function rescueTokens(address token) external onlyOwner {
        uint256 balance = IERC20(token).balanceOf(address(this));
        require(IERC20(token).transfer(owner, balance), "Transfer failed");
    }
}
