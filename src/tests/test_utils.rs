use alloy::primitives::Address;
use alloy::sol;
use pool_sync::*;
use std::sync::Arc;
use tokio::sync::broadcast;

use crate::events::Event;
use crate::swap::SwapStep;
use crate::pool_manager::PoolManager;

sol!(
    #[derive(Debug)]
    #[sol(rpc)]
    FlashQuoter,
    "src/abi/FlashQuoter.json"
);


// Load in all the pools 
pub async fn load_pools() -> (Vec<Pool>, u64) {
    dotenv::dotenv().ok();

    let pool_sync = PoolSync::builder()
        .add_pools(&[
            PoolType::UniswapV2,
        ])
        .chain(pool_sync::Chain::Base)
        .build()
        .unwrap();
    pool_sync.sync_pools().await.unwrap()
}

// construct the pool manager from working pools
pub async fn construct_pool_manager(
    pools: Vec<Pool>,
    last_synced_block: u64,
) -> (Arc<PoolManager>, broadcast::Receiver<Event>) {
    let (update_sender, update_receiver) = broadcast::channel(200);
    let pool_manager = PoolManager::new(pools, update_sender, last_synced_block).await;
    (pool_manager, update_receiver)
}


// Cosntruct a pool manager that is populated with a type of pool
pub async fn pool_manager_with_type(pool_type: PoolType) -> (Arc<PoolManager>, broadcast::Receiver<Event>) {
    dotenv::dotenv().ok();
    let pool_sync = PoolSync::builder()
        .add_pool(pool_type)
        .chain(pool_sync::Chain::Base)
        .build().unwrap();
    let (pools , last_synced_block) = pool_sync.sync_pools().await.unwrap();
    println!("Pools: {:#?}", pools.len());
    construct_pool_manager(pools, last_synced_block).await
}

// get a pool manaager that is populated wtih the pools from our address space
pub async fn pool_manager_with_pools(
    addresses: Vec<Address>,
) -> (Arc<PoolManager>, broadcast::Receiver<Event>) {
    let (pools, last_synced_block) = load_pools().await;
    let pools: Vec<Pool> = addresses
        .iter()
        .map(|address| {
            pools
                .clone()
                .into_iter()
                .find(|pool| pool.address() == *address)
                .unwrap()
        })
        .collect();

        println!("Pools: {:#?}", pools);
    let (pool_manager, reserve_receiver) =
        construct_pool_manager(pools.clone(), last_synced_block).await;
    (pool_manager, reserve_receiver)
}


// convert from our internal rep to contract rep
pub async fn swappath_to_flashquote(steps: Vec<SwapStep>) -> Vec<FlashQuoter::SwapStep> {
    steps.iter().map(|step| FlashQuoter::SwapStep {
        poolAddress: step.pool_address,
        tokenIn: step.token_in,
        tokenOut: step.token_out,
        protocol: step.as_u8(),
        fee: step.fee.try_into().unwrap(),
    }).collect()
}




