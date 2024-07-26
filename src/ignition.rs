use alloy::providers::{Provider, ProviderBuilder, RootProvider, WsConnect};
use alloy::pubsub::PubSubFrontend;
use alloy::transports::http::{Client, Http};
use log::info;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::sync::broadcast::{Receiver, Sender};

use crate::gas_manager::GasPriceManager;
use crate::graph::ArbGraph;
use crate::optimizer::optimize_paths;
use crate::pool_manager::PoolManager;
use crate::stream::*;
use crate::simulation::simulate_path;

/// Start all of the workers
pub async fn start_workers(
    http: Arc<RootProvider<Http<Client>>>,
    ws: Arc<RootProvider<PubSubFrontend>>,
    pool_manager: Arc<PoolManager>,
    graph: ArbGraph,
) {
    // all communication channels
    let (block_sender, block_receiver) = broadcast::channel(10);
    let (reserve_update_sender, reserve_update_receiver) = broadcast::channel(10);
    let (gas_sender, gas_receiver) = broadcast::channel(10);
    let (sim_sender, sim_receiver) = broadcast::channel(30);
    let (opt_sender, opt_receiver) = broadcast::channel(100);
    let (arb_sender, arb_receiver) = broadcast::channel(100);

    // graph -> tx_sender (to send tx) -> tx_receiver (opt get it)

    // Stream in new blocks
    info!("Starting block stream...");
    tokio::spawn(stream_new_blocks(ws.clone(), block_sender));

    // On each new block, parse sync events and update reserves
    info!("Starting sync event stream...");
    tokio::spawn(stream_sync_events(
        http.clone(),
        pool_manager.clone(),
        block_receiver.resubscribe(),
        reserve_update_sender,
    ));

    // Update the gas on each block
    info!("Starting gas manager...");
    let gas_manager = Arc::new(GasPriceManager::new(http.clone(), 0.1, 100));
    tokio::spawn(async move {
        gas_manager
            .update_gas_price(block_receiver.resubscribe(), gas_sender)
            .await;
    });

    info!("Starting arb simulator...");
    tokio::spawn(simulate_path(sim_sender, opt_receiver.resubscribe()));

    info!("Starting optimizer...");
    tokio::spawn(optimize_paths(opt_sender, arb_receiver.resubscribe()));

    info!("Starting transaction sender...");
    //let (tx_sender, mut tx_receiver) = broadcast::channel(1000);
    //tokio::spawn(send_transactions(
    //signer_provider,
    // gas_receiver
    //    tx_receiver.resubscribe(),
    //));

    // finally.... start the searcher!!!!!
    info!("Starting arbitrage searcher...");
    tokio::spawn(async move {
        graph
            .search_paths(arb_sender, reserve_update_receiver)
            .await;
    });
}
