use std::sync::Arc;
use log::info;
use alloy::providers::{Provider, ProviderBuilder, WsConnect, RootProvider};
use alloy::pubsub::PubSubFrontend;
use alloy::transports::http::{Client, Http};
use tokio::sync::broadcast::{Receiver, Sender};
use tokio::sync::broadcast;

use crate::stream::*;
use crate::pool_manager::PoolManager;
use crate::gas_manager::GasPriceManager;
use crate::graph::ArbGraph;




pub async fn start_workers(
    http: Arc<RootProvider<Http<Client>>>,
    ws: Arc<RootProvider<PubSubFrontend>>,
    pool_manager: Arc<PoolManager>,
    graph: ArbGraph,
) {
    // Stream in new blocks
    info!("Starting block stream...");
    let (block_sender, mut block_receiver) = broadcast::channel(10);
    tokio::spawn(stream_new_blocks(ws.clone(), block_sender));


    // On each new block, parse sync events and update reserves
    info!("Starting sync event stream...");
    let (reserve_update_sender, mut reserve_update_receiver) = broadcast::channel(10);
    tokio::spawn(stream_sync_events(
        http.clone(),
        pool_manager.clone(),
        block_receiver.resubscribe(),
        reserve_update_sender,
    ));


    // Update the gas on each block
    info!("Starting gas manager...");
    let gas_manager = Arc::new(GasPriceManager::new(http.clone(), 0.1, 100));
    let (gas_sender, mut gas_receiver) = broadcast::channel(10);
    tokio::spawn(async move {
        gas_manager
            .update_gas_price(block_receiver.resubscribe(), gas_sender)
            .await;
    });


    info!("Starting arb simulator...");
    // todo!()


    info!("Starting transaction sender...");
    //let (tx_sender, mut tx_receiver) = broadcast::channel(1000);
    //tokio::spawn(send_transactions(
    //    //signer_provider,
    //    tx_receiver.resubscribe(),
    //));

    // finally.... start the searcher!!!!!
    info!("Starting arbitrage searcher...");
    tokio::spawn(async move {
        graph.search_paths(reserve_update_receiver).await;
    });
}
