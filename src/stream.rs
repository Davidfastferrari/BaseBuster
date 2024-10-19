use crate::events::Event;
use alloy::providers::{Provider, ProviderBuilder, WsConnect};
use futures::StreamExt;
use log::{debug, warn};
use std::sync::Arc;
use std::sync::mpsc::Sender;

// stream in new blocks
pub async fn stream_new_blocks(block_sender: Sender<Event>) {
    let ws_url = WsConnect::new(std::env::var("WS").unwrap());
    let ws = Arc::new(ProviderBuilder::new().on_ws(ws_url).await.unwrap());

    let sub = ws.subscribe_blocks().await.unwrap();
    let mut stream = sub.into_stream();
    while let Some(block) = stream.next().await {
        match block_sender.send(Event::NewBlock(block)) {
            Ok(_) => debug!("Block sent"),
            Err(e) => warn!("Block send failed: {:?}", e),
        }
    }
}
