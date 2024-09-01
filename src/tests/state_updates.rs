pub mod info_sync {
    use super::*;

    #[tokio::test]
    pub async fn test_state_updates() {
        dotenv::dotenv().ok();
        let (pools, last_synced_block) = load_pools().await;
        let working_pools = get_working_pools(pools, 50, pool_sync::Chain::Base).await;
        let working_pools: Vec<Pool> = working_pools.into_iter().filter(|pool| {
            if pool.is_v3() {
                let v3_pool = pool.get_v3().unwrap();
                return v3_pool.liquidity > 0;
            };
            true
        }).collect();
        let (pool_manager, mut reserve_receiver) =
            construct_pool_manager(working_pools.clone(), last_synced_block).await;

        let mut gweiyser_pools_v2 = vec![];
        let mut gweiyser_pools_v3 = vec![];
        for pool in working_pools.iter() {
            if pool.is_v3() {
                let pool = get_v3_pool(&pool.address()).await;
                gweiyser_pools_v3.push(pool);
            } else {
                let pool = get_v2_pool(pool.address()).await;
                gweiyser_pools_v2.push(pool);
            }
        }

        while let Ok(Event::ReserveUpdate(updated_pools)) = reserve_receiver.recv().await {
            println!("Got update {}", updated_pools.len());

            for address in updated_pools.iter() {
                for pool in gweiyser_pools_v3.iter() {
                    if pool.address() == *address {
                        let slot0 = pool.slot0().await;
                        let liquidity = pool.liquidity().await;
                        let pool_manager_pool = pool_manager.get_v3pool(&pool.address());
                        
                        if slot0.tick != pool_manager_pool.tick || 
                        liquidity != pool_manager_pool.liquidity || 
                        slot0.sqrt_price_x96 != pool_manager_pool.sqrt_price {
                            println!("Mismatch found in V3 pool: {}", pool.address());
                            println!("Gweiyser pool: tick: {}, liquidity: {}, sqrt_price_x96: {}", 
                                    slot0.tick, liquidity, slot0.sqrt_price_x96);
                            println!("Pool manager pool: tick: {}, liquidity: {}, sqrt_price: {}", 
                                    pool_manager_pool.tick, pool_manager_pool.liquidity, pool_manager_pool.sqrt_price);
                            panic!("V3 pool mismatch");
                        }
                    }
                }

                for pool in gweiyser_pools_v2.iter_mut() {
                    if pool.address() == *address {
                        let reserve0 = pool.token0_reserves().await;
                        let reserve1 = pool.token1_reserves().await;
                        let pool_manager_pool = pool_manager.get_v2pool(&pool.address());
                        
                        if reserve0 != U256::from(pool_manager_pool.token0_reserves) || 
                        reserve1 != U256::from(pool_manager_pool.token1_reserves) {
                            println!("Mismatch found in V2 pool: {}", pool.address());
                            println!("Gweiyser pool: reserve0: {}, reserve1: {}", reserve0, reserve1);
                            println!("Pool manager pool: token0_reserves: {}, token1_reserves: {}", 
                                    pool_manager_pool.token0_reserves, pool_manager_pool.token1_reserves);
                            panic!("V2 pool mismatch");
                        }
                    }
                }
            }
            println!("success");
        }
    }
}
