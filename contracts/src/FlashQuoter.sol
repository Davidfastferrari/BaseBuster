// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

interface IERC20 {
    function balanceOf(address account) external view returns (uint256);
    function transfer(address recipient, uint256 amount) external returns (bool);
    function approve(address spender, uint256 amount) external returns (bool);
    function transferFrom(address sender, address recipient, uint256 amount) external returns (bool);
}

interface IUniswapV2Router {
    function swapExactTokensForTokens(
        uint256 amountIn,
        uint256 amountOutMin,
        address[] calldata path,
        address to,
        uint256 deadline
    ) external returns (uint256[] memory amounts);
}

interface IV3SwapRouterWithDeadline {
    struct ExactInputSingleParams {
        address tokenIn;
        address tokenOut;
        uint24 fee;
        address recipient;
        uint256 deadline;
        uint256 amountIn;
        uint256 amountOutMinimum;
        uint160 sqrtPriceLimitX96;
    }
    function exactInputSingle(ExactInputSingleParams calldata params) external returns (uint256 amountOut);
}

interface IV3SwapRouterNoDeadline {
    struct ExactInputSingleParams {
        address tokenIn;
        address tokenOut;
        uint24 fee;
        address recipient;
        uint256 amountIn;
        uint256 amountOutMinimum;
        uint160 sqrtPriceLimitX96;
    }
    function exactInputSingle(ExactInputSingleParams calldata params) external returns (uint256 amountOut);
}

interface IAerodromeRouter {
    struct Route {
        address from;
        address to;
        bool stable;
        address factory;
    }
    function swapExactTokensForTokens(
        uint256 amountIn,
        uint256 amountOutMin,
        Route[] calldata routes,
        address to,
        uint256 deadline
    ) external returns (uint256[] memory amounts);
}

interface IBalancerVault {
    enum SwapKind { GIVEN_IN, GIVEN_OUT }
    struct SingleSwap {
        bytes32 poolId;
        SwapKind kind;
        address assetIn;
        address assetOut;
        uint256 amount;
        bytes userData;
    }
    struct FundManagement {
        address sender;
        bool fromInternalBalance;
        address payable recipient;
        bool toInternalBalance;
    }
    function swap(
        SingleSwap memory singleSwap,
        FundManagement memory funds,
        uint256 limit,
        uint256 deadline
    ) external returns (uint256);
}

interface IPool {
    function flashLoanSimple(
        address receiverAddress,
        address asset,
        uint256 amount,
        bytes calldata params,
        uint16 referralCode
    ) external;
}

contract Quoter {
    struct SwapStep {
        address poolAddress;
        address tokenIn;
        address tokenOut;
        uint8 protocol;
        uint24 fee;
    }
    address[] private routers;

    constructor(address[] memory _routers) {
        routers = _routers;
    }

    function quote(SwapStep[] calldata steps, uint256 amount) external returns (uint256) {
        require(steps.length > 0, "No swap steps provided");
        
        IERC20 initialToken = IERC20(steps[0].tokenIn);
        require(initialToken.balanceOf(msg.sender) >= amount, "Insufficient balance");
        
        initialToken.transferFrom(msg.sender, address(this), amount);
        
        uint256 amountIn = amount;
        uint256 len = steps.length;

        for (uint256 i = 0; i < len;) {
            amountIn = _swap(steps[i], amountIn);
            unchecked { ++i; }
        }

        IERC20 finalToken = IERC20(steps[len - 1].tokenOut);
        uint256 finalBalance = finalToken.balanceOf(address(this));
        
        finalToken.transfer(msg.sender, finalBalance);

        return finalBalance;
    }

    function _swap(SwapStep memory step, uint256 amountIn) private returns (uint256) {
        IERC20(step.tokenIn).approve(routers[step.protocol], amountIn);

        if (step.protocol < 4) {
            return _swapV2(step, amountIn);
        } else if (step.protocol < 8) {
            return _swapV3(step, amountIn);
        } else if (step.protocol == 8) {
            return _swapAerodrome(step, amountIn);
        } else {
            return _swapBalancer(step, amountIn);
        }
    }

    function _swapV2(SwapStep memory step, uint256 amountIn) private returns (uint256) {
        address[] memory path = new address[](2);
        path[0] = step.tokenIn;
        path[1] = step.tokenOut;
        return IUniswapV2Router(routers[step.protocol]).swapExactTokensForTokens(
            amountIn, 0, path, address(this), block.timestamp
        )[1];
    }

    function _swapV3(SwapStep memory step, uint256 amountIn) private returns (uint256) {
        if (step.protocol < 6) {
            return IV3SwapRouterNoDeadline(routers[step.protocol]).exactInputSingle(
                IV3SwapRouterNoDeadline.ExactInputSingleParams({
                    tokenIn: step.tokenIn,
                    tokenOut: step.tokenOut,
                    fee: step.fee,
                    recipient: address(this),
                    amountIn: amountIn,
                    amountOutMinimum: 0,
                    sqrtPriceLimitX96: 0
                })
            );
        } else {
            return IV3SwapRouterWithDeadline(routers[step.protocol]).exactInputSingle(
                IV3SwapRouterWithDeadline.ExactInputSingleParams({
                    tokenIn: step.tokenIn,
                    tokenOut: step.tokenOut,
                    fee: step.fee,
                    recipient: address(this),
                    deadline: block.timestamp,
                    amountIn: amountIn,
                    amountOutMinimum: 0,
                    sqrtPriceLimitX96: 0
                })
            );
        }
    }

    function _swapAerodrome(SwapStep memory step, uint256 amountIn) private returns (uint256) {
        IAerodromeRouter.Route[] memory routes = new IAerodromeRouter.Route[](1);
        routes[0] = IAerodromeRouter.Route({
            from: step.tokenIn,
            to: step.tokenOut,
            stable: false,
            factory: address(0)
        });
        return IAerodromeRouter(routers[step.protocol]).swapExactTokensForTokens(
            amountIn, 0, routes, address(this), block.timestamp
        )[1];
    }

    function _swapBalancer(SwapStep memory step, uint256 amountIn) private returns (uint256) {
        return IBalancerVault(routers[step.protocol]).swap(
            IBalancerVault.SingleSwap({
                poolId: bytes32(uint256(uint160(step.poolAddress))),
                kind: IBalancerVault.SwapKind.GIVEN_IN,
                assetIn: step.tokenIn,
                assetOut: step.tokenOut,
                amount: amountIn,
                userData: ""
            }),
            IBalancerVault.FundManagement({
                sender: address(this),
                fromInternalBalance: false,
                recipient: payable(address(this)),
                toInternalBalance: false
            }),
            0,
            block.timestamp
        );
    }

    function rescueTokens(address token) external {
        uint256 balance = IERC20(token).balanceOf(address(this));
        require(balance > 0, "No tokens to rescue");
        IERC20(token).transfer(msg.sender, balance);
    }

    receive() external payable {}
}