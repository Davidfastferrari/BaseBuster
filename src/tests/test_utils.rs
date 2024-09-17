use alloy::transports::http::{Http, Client};
use pool_sync::*;
use std::sync::Arc;
use tokio::sync::mpsc;
use alloy::providers::{Provider, ProviderBuilder, RootProvider};
use alloy::transports::Transport;
use alloy::network::Ethereum;
use alloy::network::Network;

use crate::events::Event;
use crate::market_state::MarketState;
use crate::stream::stream_new_blocks;

// Create a market that is populated with a specific type of pool
pub async fn market_with_type(pool_type: PoolType) 
-> (Arc<MarketState<Http<Client>, Ethereum, RootProvider<Http<Client>>>>, mpsc::Receiver<Event>) {
    dotenv::dotenv().ok();

    let url = std::env::var("FULL").unwrap().parse().unwrap();
    let provider = ProviderBuilder::new().on_http(url);
    // load in all of the pools
    let pool_sync = PoolSync::builder()
        .add_pool(pool_type)
        .chain(pool_sync::Chain::Ethereum)
        .build().unwrap();
    let (pools , last_synced_block) = pool_sync.sync_pools().await.unwrap();

    // construct senders and start block stream
    let (block_tx, block_rx) = mpsc::channel::<Event>(100);
    let (address_tx, address_rx) = mpsc::channel::<Event>(100);
    tokio::spawn(stream_new_blocks(block_tx));
    
    // construct the market state
    let market_state = MarketState::init_state_and_start_stream(
        pools,
        block_rx,
        address_tx,
        last_synced_block,
        provider
    ).await.unwrap();
    (market_state, address_rx)
}

/* 
// construct the pool manager from working pools
pub async fn construct_pool_manager(
    pools: Vec<Pool>,
    last_synced_block: u64,
) -> (Arc<MarketState>, broadcast::Receiver<Event>) {
    let (update_sender, update_receiver) = broadcast::channel(200);
    let market_state = MarketState::init_state_and_start_stream(
        pools, 

        update_sender, last_synced_block).await;
    (pool_manager, update_receiver)
}

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
    */




