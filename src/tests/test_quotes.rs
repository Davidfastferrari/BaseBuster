
use super::test_utils::{load_and_filter_pools, construct_market, construct_pool_map};
use super::offchain_quote::offchain_quote;
use super::onchain_quote::onchain_quote;
use crate::events::Event;

// All offchain calculation tests
#[cfg(test)]
mod offchain_calculations {

    use pool_sync::PoolType;
    use super::*;

    macro_rules! test_pool_out {
        ($test_name:ident, $pool_type:ident) => {
            #[tokio::test(flavor = "multi_thread")]
            pub async fn $test_name() {
                dotenv::dotenv().ok();
                // load and filter pools
                let (pools, last_synced_block) = load_and_filter_pools(PoolType::$pool_type).await;
                // Pool map for references
                let pool_map = construct_pool_map(pools.clone());
                // init a market state with the new relevant pools
                let (market, address_rx) = construct_market(pools.clone(), last_synced_block).await;
                // while we get an update (new block), test onchain and offchain for all pools
                while let Ok(Event::PoolsTouched(addresses, _)) = address_rx.recv() {
                    println!("{} touched pools", addresses.len());
                    for address in addresses {
                        let pool = pool_map.get(&address).unwrap();
                        let offchain = offchain_quote(&pool, market.clone());
                        let onchain = onchain_quote(&pool).await;
                        assert_eq!(offchain, onchain, "failed with pool {:#?}", pool);
                    }
                    println!("Iteration finished");
                }
            }
        };
    }

    test_pool_out!(test_uniswapv2_out,UniswapV2);
    test_pool_out!(test_sushiswapv2_out, SushiSwapV2);
    test_pool_out!(test_pancakeswapv2_out, PancakeSwapV2);
    test_pool_out!(test_baseswapv2_out, BaseSwapV2);
    test_pool_out!(test_swapbasedv2_out, SwapBasedV2);
    test_pool_out!(test_alienbasev2_out, AlienBaseV2);
    test_pool_out!(test_dackieswapv2_out, DackieSwapV2);
    test_pool_out!(test_uniswapv3_out, UniswapV3);
    test_pool_out!(test_sushiswapv3_out, SushiSwapV3);
    test_pool_out!(test_pancakeswapv3_out, PancakeSwapV3);
    test_pool_out!(test_alienbasev3_out, AlienBaseV3);
    test_pool_out!(test_dackieswapv3_out, DackieSwapV3);
    test_pool_out!(test_swapbasedv3_out, SwapBasedV3);
    test_pool_out!(test_baseswapv3_out, BaseSwapV3);
    test_pool_out!(test_slipstream_out, Slipstream);
    test_pool_out!(test_aerodrome_out, Aerodrome);

}