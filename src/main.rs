use alloy::primitives::{Address, U256, address};
use alloy::pubsub::{PubSubConnect, PubSubFrontend};
use alloy::rpc::types::Filter;
use alloy_sol_types::{sol, SolEvent};
use alloy::network::AnyNetwork;
use futures::StreamExt;
use std::sync::Arc;
use alloy::providers::{Provider, ProviderBuilder, RootProvider, WsConnect};
use alloy::rpc::types::Log;
use log::{info, error, warn, debug};
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Sender, Receiver};
use anyhow::Result;
use env_logger;

sol!{
    #[derive(Debug)]
    #[sol(rpc)]
    contract EventDecoder {
        event Swap(address indexed,uint256,uint256,uint256,address indexed);
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();

    // construct the providers
    let ws = WsConnect::new(std::env::var("WSS_URL")?);
    let ws_provider = Arc::new(ProviderBuilder::new().network::<alloy::network::AnyNetwork>().on_ws(ws).await?);
    let http_provider = ProviderBuilder::new().network::<alloy::network::AnyNetwork>().on_http(std::env::var("HTTP_URL")?.parse()?);

    // log sender
    let (sender, mut receiver) = mpsc::channel::<Log>(1000);


    tokio::task::spawn(swap_scanner(ws_provider.clone(), sender.clone()));

    while let Some(swap_event) = receiver.recv().await {
        let decoded_log = EventDecoder::Swap::decode_log(&swap_event.inner, false);
        println!("{:?}", decoded_log);
    }
    Ok(())

}




pub async fn swap_scanner(ws_provider: Arc<RootProvider<PubSubFrontend, AnyNetwork>>, sender: Sender<Log>)  {
    let filter = Filter::new()
        .event("Swap(address,uint256,uint256,uint256,uint256,address)");

    let sub = ws_provider.subscribe_logs(&filter).await.unwrap();
    let mut stream = sub.into_stream();

    while let Some(log) = stream.next().await {
        match sender.send(log).await {
            Err(e) => error!("Unable to send the log"),
            _ => {}
        }
    }
    panic!("should not reach here");
}
