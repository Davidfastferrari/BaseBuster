use alloy::providers::ProviderBuilder;
use log::info;
use pool_sync::{Chain, Pool};
use std::sync::mpsc;
use std::thread;
use std::sync::Arc;

use crate::events::Event;
use crate::filter::filter_pools;
use crate::graph::ArbGraph;
use crate::market_state::MarketState;
use crate::searcher::Searchoor;
use crate::simulator::simulate_paths;
use crate::stream::stream_new_blocks;
use crate::tx_sender::TransactionSender;
use crate::gas_station::GasStation;

/// Start all of the workers
pub async fn start_workers(pools: Vec<Pool>, last_synced_block: u64) {
    // all of the sender and receiversb
    let (block_sender, block_receiver) = tokio::sync::broadcast::channel::<Event>(100);
    let (address_sender, address_receiver) = mpsc::channel::<Event>();
    let (paths_sender, paths_receiver) = mpsc::channel::<Event>();
    let (profitable_sender, profitable_receiver) = mpsc::channel::<Event>();

    // filter the pools here to smartly select the working set
    info!("Pool count before filter {}", pools.len());
    let pools = filter_pools(pools, 1000, Chain::Base).await;
    info!("Pool count after filter {}", pools.len());

    // start the block stream so we dont miss any blocks
    tokio::task::spawn(stream_new_blocks(block_sender));

    // Construct and start the gas station
    let gas_station = Arc::new(GasStation::new());
    tokio::spawn( {
        let gas_station = gas_station.clone();
        let block_rx = block_receiver.resubscribe();
        async move { gas_station.update_gas(block_rx).await }
    });


    // Initialize our market state, this is a wrapper over the REVM database with all our pool state
    // then start the updater
    let http_url = std::env::var("FULL").unwrap().parse().unwrap();
    let provider = ProviderBuilder::new().on_http(http_url);
    let market_state = MarketState::init_state_and_start_stream(
        pools.clone(),
        block_receiver,
        address_sender,
        last_synced_block,
        provider,
    )
    .await
    .unwrap(); // add something to reeiver blocks, this the state will be updated here

    // generate the graph
    info!("Generating cycles...");
    let cycles = ArbGraph::generate_cycles(pools.clone()).await;
    info!("Generated {} cycles", cycles.len());

    // start the simulator
    info!("Starting the simulator...");
    tokio::spawn(simulate_paths(
        profitable_sender,
        paths_receiver,
        market_state.clone(),
    ));

    // start the searcher
    info!("Starting arbitrage searcher...");
    let mut searcher = Searchoor::new(cycles, market_state.clone());
    thread::spawn(move || searcher.search_paths(paths_sender, address_receiver));

    // start the tx sender
    info!("Starting transaction sender...");
    let tx_sender = TransactionSender::new(gas_station.clone());
    tokio::spawn(async move { tx_sender.send_transactions(profitable_receiver).await });
}
