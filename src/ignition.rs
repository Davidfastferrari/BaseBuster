use alloy::primitives::Address;
use alloy::providers::ProviderBuilder;
use alloy::rpc::types::Block;
use log::info;
use pool_sync::{Chain, Pool};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::broadcast;
use std::sync::mpsc;
use std::thread;
//use tokio::sync::mpsc;

use crate::events::Event;
use crate::graph::ArbGraph;
//use crate::market::Market;
//use crate::simulator::simulate_paths;
use crate::stream::stream_new_blocks;
//use crate::swap::SwapStep;
//use crate::tx_sender::TransactionSender;
use crate::filter::filter_pools;
use crate::market_state::MarketState;
use crate::searcher::Searchoor;

/// Start all of the workers
pub async fn start_workers(pools: Vec<Pool>, last_synced_block: u64) {
    // all of the sender and receivers
    let (block_sender, block_receiver) = mpsc::channel::<Event>();
    let (address_sender, address_receiver) = mpsc::channel::<Event>();
    let (paths_sender, paths_receiver) = mpsc::channel::<Event>();
    let (profitable_sender, profitable_receiver) = mpsc::channel::<Event>();

    // filter the pools here to smartly select the working set
    info!("Pool count before filter {}", pools.len());
    let pools = filter_pools(pools, 500, Chain::Ethereum).await;
    info!("Pool count after filter {}", pools.len());

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

    // start our block reciever
    // Stream in new blocks
    info!("Starting block stream...");
    tokio::spawn(stream_new_blocks(block_sender));

    // generate the graph
    info!("Generating cycles...");
    let cycles = ArbGraph::generate_cycles(pools.clone()).await;
    info!("Generated {} cycles", cycles.len());

    // finally.... start the searcher!!!!!
    info!("Starting arbitrage searcher...");
    let mut searcher = Searchoor::new(cycles, market_state.clone());
    thread::spawn(move || { searcher.search_paths(paths_sender, address_receiver)});
    // start the simulator
    // start the sender
    // start the searcher

    //  Create a calculator with the pool state
    //let calculator = Calculator::new(market_state.clone());

    /*
        // all communication channels
        let (reserve_update_sender, reserve_update_receiver) = broadcast::channel(10);
        let (arb_sender, arb_receiver) = mpsc::channel();
        let (tx_sender, tx_receiver) = mpsc::channel();

        // get out working pools and construct ethe pool manager
        info!("Getting working pools...");
        let num_tokens: usize = std::env::var("NUM_TOKENS").unwrap().parse().unwrap();
        let working_pools = get_working_pools(pools.clone(), num_tokens, Chain::Base).await;
        let filtered_pools: Vec<Pool> = working_pools.into_iter().filter(|pool| {
            if pool.is_v3() {
                let v3_pool = pool.get_v3().unwrap();
                return v3_pool.liquidity > 0;
            };
            // keep all other pools
            true
        }).collect();
        let pool_manager = PoolManager::new(filtered_pools.clone(), reserve_update_sender.clone(), last_synced_block).await;


        // simulate arbitrage paths in a new thread
        info!("Starting simulator...");
        std::thread::spawn(move || {
            simulate_paths(tx_sender, arb_receiver);
        });

        // transaction sender
        info!("Starting transaction sender...");
        let tx_sender = TransactionSender::new(market);
        tokio::spawn(async move {
            let _ = tx_sender.send_transactions(tx_receiver).await;
        });

    */
}
