// SPDX-License-Identifier: MIT
pragma solidity ^0.8.25;

interface v2Pair {
    function getReserves() external view returns (uint112 reserve0, uint112 reserve1, uint32 blockTimestamp);
}

interface v3Pool {
    function slot0() external view returns (
        uint160 sqrtPriceX96,
        int24 tick,
        uint16 observationIndex,
        uint16 observationCardinality,
        uint16 observationCardinalityNext,
        uint8 feeProtocol,
        bool unlocked
    );
    function liquidity() external view returns (uint128);
}

contract BatchSync {
    struct V2PairReserves {
        address pairAddr;
        uint112 reserve0;
        uint112 reserve1;
    }

    struct V3PoolState {
        address poolAddr;
        int24 tick;
        uint128 liquidity;
        uint160 sqrtPriceX96;
    }

    function syncV2(address[] calldata v2Pools) external view returns (V2PairReserves[] memory) {
        V2PairReserves[] memory reserves = new V2PairReserves[](v2Pools.length);
        for (uint i = 0; i < v2Pools.length; i++) {
            reserves[i].pairAddr = v2Pools[i];
            (reserves[i].reserve0, reserves[i].reserve1,) = v2Pair(v2Pools[i]).getReserves();
        }
        return reserves;
    }

    function syncV3(address[] calldata pools) external view returns (V3PoolState[] memory) {
        V3PoolState[] memory states = new V3PoolState[](pools.length);
        for (uint i = 0; i < pools.length; i++) {
            states[i].poolAddr = pools[i];
            (
                states[i].sqrtPriceX96,
                states[i].tick,
                , // observationIndex
                , // observationCardinality
                , // observationCardinalityNext
                , // feeProtocol
                  // unlocked
            ) = v3Pool(pools[i]).slot0();
            states[i].liquidity = v3Pool(pools[i]).liquidity();
        }
        return states;
    }

}