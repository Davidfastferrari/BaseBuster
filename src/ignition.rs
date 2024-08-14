use alloy::primitives::address;
use alloy::providers::RootProvider;
use alloy::pubsub::PubSubFrontend;
use alloy::transports::http::{Client, Http};
use log::info;
use pool_sync::{Chain, Pool};
use std::sync::Arc;
use tokio::sync::broadcast;

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
    last_synced_block: u64,
) {
    // all communication channels
    let (block_sender, block_receiver) = broadcast::channel(10);
    let (reserve_update_sender, reserve_update_receiver) = broadcast::channel(10);
    let (arb_sender, arb_receiver) = broadcast::channel(1000);
    let (tx_sender, tx_receiver) = broadcast::channel(1000);

    // get out working pools and construct ethe pool manager
    info!("Getting working pools...");
    let working_pools = get_working_pools(pools.clone(), 5000, Chain::Base).await;
    let pool_manager = PoolManager::new(pools.clone(), reserve_update_sender.clone(), last_synced_block).await;

    // construct the graph and generate the cycles
    info!("Constructing graph...");
    let weth = address!("4200000000000000000000000000000000000006");
    let graph = ArbGraph::new(pool_manager.clone(), working_pools.clone(), weth);

    // Stream in new blocks
    info!("Starting block stream...");
    tokio::spawn(stream_new_blocks(ws.clone(), block_sender));


    // Market state
    info!("Staring market state tracker...");
    let market = Arc::new(Market::new());
    let market_clone = market.clone();
    tokio::spawn(async move {
        market_clone
            .update_gas_price(block_receiver.resubscribe())
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

