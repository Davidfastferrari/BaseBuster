use alloy::providers::{Provider, RootProvider};
use alloy::pubsub::PubSubFrontend;
use alloy::rpc::types::Filter;
use alloy::sol;
use alloy::transports::http::{Client, Http};
use alloy::primitives::U128;
use alloy_sol_types::SolEvent;
use futures::StreamExt;
use log::{info, debug};
use std::sync::Arc;
use tokio::sync::broadcast::{Receiver, Sender};

use crate::events::Event;
use crate::pool_manager::PoolManager;

// The sync event is emitted whenever a pool is synced
sol!(
    #[derive(Debug)]
    contract SyncEvent {
        event Sync(uint112 reserve0, uint112 reserve1);
    }
);

sol! {
    #[derive(Debug)]
    contract UniswapV3Events {
        event Swap(
            address indexed sender,
            address indexed recipient,
            int256 amount0,
            int256 amount1,
            uint160 sqrtPriceX96,
            uint128 liquidity,
            int24 tick
        );

        event Mint(
            address sender,
            address indexed owner,
            int24 indexed tickLower,
            int24 indexed tickUpper,
            uint128 amount,
            uint256 amount0,
            uint256 amount1
        );

        event Burn(
            address indexed owner,
            int24 indexed tickLower,
            int24 indexed tickUpper,
            uint128 amount,
            uint256 amount0,
            uint256 amount1
        );
    }
}

// stream in new blocks
pub async fn stream_new_blocks(ws: Arc<RootProvider<PubSubFrontend>>, block_sender: Sender<Event>) {
    let sub = ws.subscribe_blocks().await.unwrap();
    let mut stream = sub.into_stream();
    while let Some(block) = stream.next().await {
        info!("New block: {:?}", block.header.number.unwrap());
        match block_sender.send(Event::NewBlock(block)) {
            Ok(_) => info!("Block sent"),
            Err(e) => info!("Block send failed: {:?}", e),
        }
    }
}

// on each block update, get all the sync events and update pool reserves
pub async fn stream_sync_events(
    http: Arc<RootProvider<Http<Client>>>, // the http provider to fetch logs from
    pool_manager: Arc<PoolManager>,        // mapping of the pools we are seaching over
    mut block_receiver: Receiver<Event>,   // block receiver
    reserve_update_sender: Sender<Event>,  // reserve update sender
) {
    // wait for a new block
    while let Ok(Event::NewBlock(block)) = block_receiver.recv().await {
        // create our filter for the sync events
        let v2_filter = Filter::new()
            .event(SyncEvent::Sync::SIGNATURE)
            .from_block(block.header.number.unwrap());

        let v3_filter = Filter::new()
            .events([
                UniswapV3Events::Swap::SIGNATURE,
                UniswapV3Events::Mint::SIGNATURE,
                UniswapV3Events::Burn::SIGNATURE,
            ])
            .from_block(block.header.number.unwrap());

        // fetch all the logs
        info!("Fetching logs...");
        let (v2_logs, v3_logs) = tokio::join!(http.get_logs(&v2_filter), http.get_logs(&v3_filter));

        if let Ok(v2_logs) = v2_logs {
            // update all the pool reserves based on the sync events
            for log in v2_logs {
                let decoded_log = SyncEvent::Sync::decode_log(&log.inner, false).unwrap();
                let pool_address = decoded_log.address;
                let SyncEvent::Sync { reserve0, reserve1 } = decoded_log.data;
    
                // update the reserves if we are tracking the pool
                if pool_manager.exists(&pool_address) {
                    debug!("Found v2 log for pool {:?}", pool_address);
                    pool_manager.update_v2(pool_address, reserve0, reserve1);
                }
            }
        }

        if let Ok(v3_logs) = v3_logs {
            for log in v3_logs {
                let pool_address = log.address();
                if pool_manager.exists(&pool_address) {
                    debug!("Found v3 log for pool {:?}", pool_address);
                    if let Ok(swap_event) = UniswapV3Events::Swap::decode_log(&log.inner, false) {
                        let UniswapV3Events::Swap { sqrtPriceX96, liquidity, tick, .. } = swap_event.data;
                        pool_manager.update_v3(pool_address, sqrtPriceX96, tick, U128::from(liquidity));
                    } else if let Ok(mint_event) = UniswapV3Events::Mint::decode_log(&log.inner, false) {
                        // For Mint events, we need to update the liquidity
                        let UniswapV3Events::Mint { amount, .. } = mint_event.data;
                        let (current_sqrt_price, current_tick, current_liquidity) = pool_manager.get_v3(&pool_address);
                        let new_liquidity = current_liquidity.saturating_add(U128::from(amount));
                        pool_manager.update_v3(pool_address, current_sqrt_price, current_tick, new_liquidity);
                    } else if let Ok(burn_event) = UniswapV3Events::Burn::decode_log(&log.inner, false) {
                        // For Burn events, we need to update the liquidity
                        let UniswapV3Events::Burn { amount, .. } = burn_event.data;
                        let (current_sqrt_price, current_tick, current_liquidity) = pool_manager.get_v3(&pool_address);
                        let new_liquidity = current_liquidity.saturating_sub(U128::from(amount));
                        pool_manager.update_v3(pool_address, current_sqrt_price, current_tick, new_liquidity);
                    }
                }
            }
        }

        // send notification saying that we have updated the reserves
        match reserve_update_sender.send(Event::ReserveUpdate) {
            Ok(_) => info!("Reserves updated"),
            Err(e) => info!("Reserves update failed: {:?}", e),
        }
    }
}
























