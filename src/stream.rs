use alloy::providers::{Provider, RootProvider};
use alloy::pubsub::PubSubFrontend;
use futures::StreamExt;
use log::{debug, info, warn};
use std::sync::Arc;
use tokio::sync::broadcast::Sender;

use crate::events::Event;

// stream in new blocks
pub async fn stream_new_blocks(ws: Arc<RootProvider<PubSubFrontend>>, block_sender: Sender<Event>) {
    let sub = ws.subscribe_blocks().await.unwrap();
    let mut stream = sub.into_stream();
    while let Some(block) = stream.next().await {
        info!("New block: {:?}", block.header.number.unwrap());
        match block_sender.send(Event::NewBlock(block)) {
            Ok(_) => debug!("Block sent"),
            Err(e) => warn!("Block send failed: {:?}", e),
        }
    }
}
