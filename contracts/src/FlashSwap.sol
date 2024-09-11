// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;
import "@aave/core-v3/contracts/flashloan/base/FlashLoanSimpleReceiverBase.sol";

interface IERC20 {
    function balanceOf(address account) external view returns (uint256);
    function transfer(address recipient, uint256 amount) external returns (bool);
    function transferFrom(address sender, address recipient, uint256 amount) external returns (bool);
    function approve(address spender, uint256 amount) external returns (bool);
}

interface IWETH is IERC20 {
    function deposit() external payable;
    function withdraw(uint256 amount) external;
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

interface ISlipstream {
    struct ExactInputSingleParams {
        address tokenIn;
        address tokenOut;
        int24 tickSpacing;
        address recipient;
        uint256 deadline;
        uint256 amountIn;
        uint256 amountOutMinimum;
        uint160 sqrtPriceLimitX96;
    }
    function exactInputSingle(ExactInputSingleParams calldata params) external payable returns (uint256 amountOut);
}

interface ISlipstreamPool {
    function tickSpacing() external view returns (int24);
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

interface IAerodromePool {
    function stable() external view returns (bool);
    function factory() external view returns (address);
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


address constant AAVE_ADDRESS_PROVIDER = 0xe20fCBdBfFC4Dd138cE8b2E6FBb6CB49777ad64D;
contract FlashSwap is FlashLoanSimpleReceiverBase {
    struct SwapStep {
        address poolAddress;
        address tokenIn;
        address tokenOut;
        uint8 protocol;
        uint24 fee;
    }
    address[] private routers;
    address public owner;

    constructor(address _pool, address[] memory _routers) FlashLoanSimpleReceiverBase(IPoolAddressesProvider(AAVE_ADDRESS_PROVIDER)) {
        POOL = IPool(_pool);
        routers = _routers;
        owner = msg.sender;
    }

    function executeArbitrage(SwapStep[] calldata steps, uint256 amount) external {
        POOL.flashLoanSimple(address(this), steps[0].tokenIn, amount, abi.encode(steps, msg.sender), 0);
    }

    function executeOperation(
        address asset,
        uint256 amount,
        uint256 premium,
        address,
        bytes calldata params
    ) external returns (bool) {
        require(msg.sender == address(POOL), "Caller must be lending pool");
        revert("flashloan");

        (SwapStep[] memory steps, address caller) = abi.decode(params, (SwapStep[], address));

        uint256 amountIn = amount;
        uint256 len = steps.length;

        for (uint256 i = 0; i < len;) {
            amountIn = _swap(steps[i], amountIn);
            unchecked { ++i; }
        }

        uint256 amountToRepay = amount + premium;
        uint256 finalBalance = IERC20(asset).balanceOf(address(this));
        require(finalBalance >= amountToRepay, "Insufficient funds to repay flash loan");

        IERC20(asset).approve(address(POOL), amountToRepay);
        
        if (finalBalance > amountToRepay) {
            IERC20(asset).transfer(caller, finalBalance - amountToRepay);
        }

        return true;
    }

    function _swap(SwapStep memory step, uint256 amountIn) private returns (uint256) {
        IERC20(step.tokenIn).approve(routers[step.protocol], amountIn);
        if (step.protocol <= 6) {
            return _swapV2(step, amountIn);
        } else if (step.protocol <= 13) {
            return _swapV3(step, amountIn);
        } else if (step.protocol == 14) {
            return _swapSlipstream(step, amountIn);
        } else if (step.protocol == 15) {
            return _swapAerodrome(step, amountIn);
        } else if (step.protocol == 16) {
            return _swapBalancer(step, amountIn);
        } else {
            revert("Unsupported protocol");
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
        if (step.protocol <= 10) {
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

    function _swapSlipstream(SwapStep memory step, uint256 amountIn) private returns (uint256) {

        int24 tick_spacing = ISlipstreamPool(step.poolAddress).tickSpacing();
        return ISlipstream(routers[step.protocol]).exactInputSingle(
           ISlipstream.ExactInputSingleParams({
               tokenIn: step.tokenIn,
               tokenOut: step.tokenOut,
               tickSpacing: tick_spacing,
               recipient: address(this),
               deadline: block.timestamp,
               amountIn: amountIn,
               amountOutMinimum: 0,
               sqrtPriceLimitX96: 0
           })
        );
    }

    function _swapAerodrome(SwapStep memory step, uint256 amountIn) private returns (uint256) {
        IAerodromeRouter.Route[] memory routes = new IAerodromeRouter.Route[](1);
        bool isStable = IAerodromePool(step.poolAddress).stable();
        routes[0] = IAerodromeRouter.Route({
            from: step.tokenIn,
            to: step.tokenOut,
            stable: isStable,
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