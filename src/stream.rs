use alloy::primitives::BlockNumber;
use alloy::providers::{Provider, RootProvider};
use alloy::transports::http::{Http, Client};
use alloy_sol_types::SolEvent;
use std::sync::Arc;
use alloy::eips::BlockId;
use pool_sync::PoolInfo;
use log::info;
use alloy::pubsub::PubSubFrontend;
use alloy::rpc::types::Filter;
use tokio::sync::mpsc::Sender;
use alloy::sol;
use futures::StreamExt;
use crate::concurrent_pool::ConcurrentPool;
use crate::events::Events;

sol!(
    #[derive(Debug)]
    contract SyncEvent {
        event Sync(uint112 reserve0, uint112 reserve1);
    }
);

// Stream all of the new blocks
pub async fn stream_sync_events(
    ws: Arc<RootProvider<PubSubFrontend>>,
    http: Arc<RootProvider<Http<Client>>>,
    tracked_pools: Arc<ConcurrentPool>,
    new_log_sender: Sender<Events>
) {
    let sub = ws.subscribe_blocks().await.unwrap();
    let mut stream = sub.into_stream();
    while let Some(block) = stream.next().await {
        info!("New block: {:?}", block.header.number.unwrap());
        let filter = Filter::new()
            .event(SyncEvent::Sync::SIGNATURE)
            .from_block(block.header.number.unwrap());
    
        info!("Fetching logs...");
        let logs = http.get_logs(&filter).await.unwrap();
        info!("Updating reserves...");
        for log in logs {
            let decoded_log = SyncEvent::Sync::decode_log(&log.inner, false).unwrap();
            let pool_address = decoded_log.address;
            let SyncEvent::Sync {reserve0, reserve1} = decoded_log.data;

            // update the reserves if we are tracking the pool
            if tracked_pools.exists(&pool_address) {
                tracked_pools.update(&pool_address, reserve0, reserve1);
            }
        }
        info!("Reserves updates");
        let _ = new_log_sender.send(Events::ReserveUpdate).await;
    }
}