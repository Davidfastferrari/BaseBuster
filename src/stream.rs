use alloy::primitives::U128;
use alloy::providers::{Provider, RootProvider};
use alloy::pubsub::PubSubFrontend;
use alloy::rpc::types::Filter;
use alloy::primitives::Address;
use alloy::sol;
use alloy::transports::http::{Client, Http};
use alloy_sol_types::SolEvent;
use futures::StreamExt;
use log::{debug, info};
use std::sync::Arc;
use tokio::sync::broadcast::{Receiver, Sender};
use pool_sync::snapshot::*;

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
pub async fn state_updater(
    http: Arc<RootProvider<Http<Client>>>, // the http provider to fetch logs from
    pool_manager: Arc<PoolManager>,        // mapping of the pools we are seaching over
    mut block_receiver: Receiver<Event>,   // block receiver
    reserve_update_sender: Sender<Event>,  // reserve update sender
) {
    // wait for a new block
    while let Ok(Event::NewBlock(block)) = block_receiver.recv().await {
        let block_number = block.header.number.unwrap();
        
        let (v2_filter, v3_filter) = (
            Filter::new().event(SyncEvent::Sync::SIGNATURE).from_block(block_number),
            Filter::new().events([
                UniswapV3Events::Swap::SIGNATURE,
                UniswapV3Events::Mint::SIGNATURE,
                UniswapV3Events::Burn::SIGNATURE,
            ]).from_block(block_number)
        );

        info!("Fetching logs...");
        let (v2_logs, v3_logs) = tokio::join!(
            http.get_logs(&v2_filter),
            http.get_logs(&v3_filter)
        );

        let (v2_addresses, v3_addresses): (Vec<Address>, Vec<Address>) = (
            v2_logs.unwrap_or_default().into_iter()
                .filter_map(|log| SyncEvent::Sync::decode_log(&log.inner, false).ok())
                .map(|decoded| decoded.address)
                .filter(|addr| pool_manager.exists(addr))
                .collect(),
            v3_logs.unwrap_or_default().into_iter()
                .map(|log| log.address())
                .filter(|addr| pool_manager.exists(addr))
                .collect()
        );
        let v2_snapshots = v2_pool_snapshot(v2_addresses.clone(), http.clone()).await;
        let v3_snapshots = v3_pool_snapshot(&v3_addresses.clone(), http.clone()).await;

        pool_manager.v2_update_from_snapshots(v2_snapshots.unwrap());
        pool_manager.v3_update_from_snapshots(v3_snapshots.unwrap());


        // send notification saying that we have updated the reserves
        let updated_pools = vec![v2_addresses, v3_addresses].into_iter().flatten().collect();

        match reserve_update_sender.send(Event::ReserveUpdate(updated_pools)) {
            Ok(_) => info!("Reserves updated"),
            Err(e) => info!("Reserves update failed: {:?}", e),
        }
    }
}
