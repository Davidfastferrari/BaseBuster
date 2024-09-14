use log::info;
use pool_sync::{Chain, Pool};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::broadcast;
use std::collections::HashSet;
use alloy::primitives::Address;
use alloy::rpc::types::Block;
use std::sync::mpsc;
//use tokio::sync::mpsc;

use crate::graph::ArbGraph;
use crate::market::Market;
use crate::pool_manager::PoolManager;
use crate::simulator::simulate_paths;
use crate::stream::*;
use crate::swap::SwapStep;
use crate::tx_sender::TransactionSender;
use crate::util::get_working_pools;
use crate::searcher::Searchoor;
use crate::market_state::MarketState;

/// Start all of the workers
pub async fn start_workers(
    pools: Vec<Pool>,
    last_synced_block: u64,
) {
    // all of the sender and receivers
    let (block_tx, block_rx) = mpsc::channel::<Block>(100);
    let (address_tx, address_rx) = mpsc::channel::<HashSet<Address>>(100);
    let (paths_tx, paths_rx) = mpsc::channel::<Vec<Vec<SwapStep>>>(1000);
    let (profitable_tx, profitable_rx) = mpsc::channel::<Vec<Vec<SwapStep>>>(100);

    // filter the pools here to smartly select the working set
    // let pools = filter_pools(pools);

    // Initialize our market state, this is a wrapper over the REVM database with all our pool state
    // then start he updater
    let market_state = MarketState::init_state_and_start_stream(
        pools.clone(),
        block_rx, 
        address_tx
    ).await.unwrap(); // add something to reeiver blocks, this the state will be updated here


    // start our block reciever
    // Stream in new blocks
    info!("Starting block stream...");
    tokio::spawn(stream_new_blocks(block_tx));

    // generate the graph
    let cycles = ArbGraph::generate_cycles(pools.clone()).await;

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

    // construct the graph and generate the cycles
    info!("Constructing graph...");
    let arb_token = std::env::var("ARB_TOKEN").unwrap().parse().unwrap();
    let cycles = ArbGraph::generate_cycles(filtered_pools.clone(), arb_token).await;
    println!("found {}", cycles.len());

    // Stream in new blocks
    info!("Starting block stream...");
    tokio::spawn(stream_new_blocks(block_sender));

    // Market state
    info!("Staring market state tracker...");
    let market = Arc::new(Market::new());
    let market_clone = market.clone();
    tokio::spawn(async move {
        market_clone
            .update_gas_price(block_receiver.resubscribe())
            .await;
    });

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

    // finally.... start the searcher!!!!!
    info!("Starting arbitrage searcher...");
    let mut searcher = Searchoor::new(cycles, pool_manager).await;
    tokio::spawn(async move {
        searcher.search_paths(
            arb_sender, 
            reserve_update_receiver.resubscribe()
        ).await
    });
*/
}

