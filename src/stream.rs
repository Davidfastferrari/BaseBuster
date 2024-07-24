use alloy::providers::{Provider, RootProvider};
use tokio::sync::broadcast::{Receiver, Sender};
use alloy::transports::http::{Client, Http};
use alloy::pubsub::PubSubFrontend;
use alloy::rpc::types::Filter;
use alloy_sol_types::SolEvent;
use futures::StreamExt;
use std::sync::Arc;
use alloy::sol;
use log::info;

use crate::pool_manager::PoolManager;
use crate::events::Event;

// The sync event is emitted whenever a pool is synced
sol!(
    #[derive(Debug)]
    contract SyncEvent {
        event Sync(uint112 reserve0, uint112 reserve1);
    }
);

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
    pool_manager: Arc<PoolManager>,    // mapping of the pools we are seaching over
    mut block_receiver: Receiver<Event>,   // block receiver
    reserve_update_sender: Sender<Event>,  // reserve update sender
) {
    // wait for a new block
    while let Ok(Event::NewBlock(block)) = block_receiver.recv().await {
        // create our filter for the sync events
        let filter = Filter::new()
            .event(SyncEvent::Sync::SIGNATURE)
            .from_block(block.header.number.unwrap());

        // fetch all the logs
        info!("Fetching logs...");
        let logs = http.get_logs(&filter).await.unwrap();

        // update all the pool reserves based on the sync events
        info!("Updating reserves...");
        for log in logs {
            let decoded_log = SyncEvent::Sync::decode_log(&log.inner, false).unwrap();
            let pool_address = decoded_log.address;
            let SyncEvent::Sync { reserve0, reserve1 } = decoded_log.data;

            // update the reserves if we are tracking the pool
            if pool_manager.exists(&pool_address) {
                pool_manager.update_reserves(pool_address, reserve0, reserve1);
            }
        }

        // send notification saying that we have updated the reserves
        match reserve_update_sender.send(Event::ReserveUpdate) {
            Ok(_) => info!("Reserves updated"),
            Err(e) => info!("Reserves update failed: {:?}", e),
        }
    }
}
