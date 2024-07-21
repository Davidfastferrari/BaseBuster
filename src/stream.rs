use alloy::providers::{Provider, RootProvider};
use alloy_sol_types::SolEvent;
use alloy::pubsub::PubSubFrontend;
use alloy::rpc::types::Filter;
use alloy::sol;
use futures::StreamExt;
use std::sync::Arc;

sol!(
    #[derive(Debug)]
    contract SyncEvent {
        event Sync(uint112 reserve0, uint112 reserve1);
    }
);

pub async fn stream_blocks(ws: Arc<RootProvider<PubSubFrontend>>) {
    let filter = Filter::new().event(SyncEvent::Sync::SIGNATURE);

    let sub = ws.subscribe_logs(&filter).await.unwrap();
    let mut stream = sub.into_stream();

    while let Some(log) = stream.next().await {
        let data = SyncEvent::Sync::decode_log(&log.inner, false);
        // if address in our pools, need to update it
        println!("{:?}", data);
    }
}
