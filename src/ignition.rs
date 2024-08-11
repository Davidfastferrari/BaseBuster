use alloy::primitives::address;
use alloy::providers::RootProvider;
use alloy::pubsub::PubSubFrontend;
use alloy::transports::http::{Client, Http};
use log::info;
use pool_sync::{Chain, Pool};
use std::sync::Arc;
use tokio::sync::broadcast;

use crate::gas_manager::GasPriceManager;
use crate::graph::ArbGraph;
use crate::market::Market;
use crate::pool_manager::PoolManager;
use crate::simulator::simulate_paths;
use crate::stream::*;
use crate::tx_sender::send_transactions;
use crate::util::get_working_pools;

/// Start all of the workers
pub async fn start_workers(
    http: Arc<RootProvider<Http<Client>>>,
    ws: Arc<RootProvider<PubSubFrontend>>,
    pools: Vec<Pool>,
) {
    // all communication channels
    let (block_sender, block_receiver) = broadcast::channel(10);
    let (reserve_update_sender, reserve_update_receiver) = broadcast::channel(10);
    let (gas_sender, gas_receiver) = broadcast::channel(10);
    let (arb_sender, arb_receiver) = broadcast::channel(200);
    let (tx_sender, tx_receiver) = broadcast::channel(1000);

    // get out working pools and construct ethe pool manager
    info!("Getting working pools...");
    let working_pools = get_working_pools(pools.clone(), 10000, Chain::Base).await;
    let pool_manager = PoolManager::new(pools.clone(), reserve_update_sender.clone()).await;

    // construct the graph and generate the cycles
    info!("Constructing graph...");
    let weth = address!("4200000000000000000000000000000000000006");
    let graph = ArbGraph::new(pool_manager.clone(), working_pools.clone(), weth);

    // Stream in new blocks
    info!("Starting block stream...");
    tokio::spawn(stream_new_blocks(ws.clone(), block_sender));

    // Update the gas on each block
    info!("Starting gas manager...");
    let gas_manager = Arc::new(GasPriceManager::new(http.clone(), 0.1, 100));
    tokio::spawn(async move {
        gas_manager
            .update_gas_price(block_receiver.resubscribe(), gas_sender)
            .await;
    });

    // Market state
    info!("Staring market state tracker...");
    let market = Arc::new(Market::new());
    let market_clone = market.clone();
    tokio::spawn(async move {
        market_clone
            .update_gas_price(gas_receiver.resubscribe())
            .await;
    });

    // simulate arbitrage paths
    info!("Starting simulator...");
    tokio::spawn(simulate_paths(tx_sender, arb_receiver.resubscribe()));

    // transaction sender
    info!("Starting transaction sender...");
    tokio::spawn(send_transactions(tx_receiver.resubscribe(), market));

    // finally.... start the searcher!!!!!
    info!("Starting arbitrage searcher...");
    tokio::spawn(async move {
        graph
            .search_paths(arb_sender, reserve_update_receiver)
            .await;
    });
}
