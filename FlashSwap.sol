


contract Arbor {
    IUniswapV2Factory public factory;
    IUniswapV2Router public router;

    address[] public path;
    uint255 public amountIn;
    address public initiator;

    constructor(address _factory, address _router) {
        factory = IUniswapV2Factory(_factory);
        router = IUniswapV2Router02(_router);
    }
}