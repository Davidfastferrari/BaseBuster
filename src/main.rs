//use crate::build_graph::{construct_graph, find_best_arbitrage_path};
use crate::concurrent_pool::ConcurrentPool;
use crate::events::Event;
use crate::gas_manager::GasPriceManager;
use crate::graph::*;
use crate::tx_sender::send_transactions;
use alloy::hex::FromHex;
use alloy::network::EthereumWallet;
use alloy::primitives::address;
use alloy::primitives::Address;
use alloy::primitives::{FixedBytes, U128, U256};
use alloy::providers::ProviderBuilder;
use alloy::providers::WsConnect;
use alloy::signers::local::PrivateKeySigner;
use alloy::sol;
use env_logger;
use log::info;
use log::LevelFilter;
use petgraph::algo;
use petgraph::prelude::*;
use pool_sync::filter::filter_top_volume;
use pool_sync::*;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::sync::Arc;
use stream::*;
use tokio::sync::broadcast;
use tokio::sync::mpsc;

mod concurrent_pool;
mod events;
mod gas_manager;
mod graph;
//mod calculation;
mod market;
mod optimizer;
mod stream;
mod tx_sender;

sol!(
    #[derive(Debug)]
    #[sol(rpc)]
    contract UniswapV2Router {
        function getAmountsOut(uint amountIn, address[] memory path) external view returns (uint[] memory amounts);
    }
);

// Pair contract to get reserves
sol!(
    #[derive(Debug)]
    #[sol(rpc)]
    contract UniswapV2Pair {
        function reserves() external view returns (uint112 reserve0, uint112 reserve1);
    }
);

// function that will take in a pool and update the reserves address and return the reserves

#[derive(Serialize, Deserialize)]
struct AddressSet(HashSet<Address>);
fn write_addresses_to_file(addresses: &HashSet<Address>, filename: &str) -> std::io::Result<()> {
    let file = File::create(filename)?;
    let writer = BufWriter::new(file);
    let address_set = AddressSet(addresses.clone());
    serde_json::to_writer(writer, &address_set)?;
    Ok(())
}

fn read_addresses_from_file(filename: &str) -> std::io::Result<HashSet<Address>> {
    let file = File::open(filename)?;
    let reader = BufReader::new(file);
    let address_set: AddressSet = serde_json::from_reader(reader)?;
    Ok(address_set.0)
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    // initializations
    dotenv::dotenv().ok();
    env_logger::Builder::new()
        .filter_level(LevelFilter::Info) // or Info, Warn, etc.
        .init();

    let private_key_hex = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

    // Convert the hex string to FixedBytes<32>
    let private_key_bytes = FixedBytes::<32>::from_hex(private_key_hex).unwrap();

    // Create a PrivateKeySigner from the private key bytes
    let signer = PrivateKeySigner::from_bytes(&private_key_bytes).unwrap();
    let wallet = EthereumWallet::from(signer);

    // construct the providers
    info!("Constructing providers...");
    let http_url = std::env::var("HTTP").unwrap();
    let ws_url = WsConnect::new(std::env::var("WS").unwrap());
    let http_provider = Arc::new(ProviderBuilder::new().on_http(http_url.parse().unwrap()));
    let signer_provider = Arc::new(
        ProviderBuilder::new()
            .wallet(wallet)
            .on_http("http://localhost:8545".parse().unwrap()),
    );
    let ws_provider = Arc::new(ProviderBuilder::new().on_ws(ws_url).await.unwrap());

    // load in all the pools
    info!("Loading pools...");
    let pool_sync = PoolSync::builder()
        .add_pools(&[PoolType::UniswapV2])
        .chain(Chain::Ethereum)
        .rate_limit(100)
        .build()
        .unwrap();

    // sync all the pools and get the top tokens
    info!("Syncing pools...");
    let pools = pool_sync.sync_pools(http_provider.clone()).await.unwrap();
    info!("Loading top volume tokens...");
    let top_volume_tokens = read_addresses_from_file("addresses.json")?;

    // all our mappings
    let mut address_to_pool = ConcurrentPool::new(); // for pool data
    let mut address_to_node: FxHashMap<Address, NodeIndex> = FxHashMap::default(); // for finding node idx
    let mut token_to_edge: FxHashMap<(NodeIndex, NodeIndex), EdgeIndex> = FxHashMap::default();

    // build the graph and populate the mappings
    let graph = Arc::new(build_graph(
        &pools,
        top_volume_tokens,
        &mut address_to_node,
        &mut address_to_pool,
        &mut token_to_edge,
    ));
    address_to_pool.sync_pools(http_provider.clone()).await;

    // rewrap it
    let address_to_pool = Arc::new(address_to_pool);

    // fetch the weth node index
    let node = *address_to_node
        .get(&address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"))
        .unwrap();

    // build all of the cycles
    info!("Building cycles...");
    let cycles = construct_cycles(&graph, node);
    info!("Found {} cycles", cycles.len());

    // block stream
    let (block_sender, mut block_receiver) = broadcast::channel(10);
    tokio::spawn(stream_new_blocks(ws_provider.clone(), block_sender));

    // sync events stream
    let (reserve_update_sender, mut reserve_update_receiver) = broadcast::channel(10);
    tokio::spawn(stream_sync_events(
        http_provider.clone(),
        address_to_pool.clone(),
        block_receiver.resubscribe(),
        reserve_update_sender,
    ));

    // start the gas manager
    let gas_manager = Arc::new(GasPriceManager::new(http_provider.clone(), 0.1, 100));
    let (gas_sender, mut gas_receiver) = broadcast::channel(10);
    tokio::spawn(async move {
        gas_manager
            .update_gas_price(block_receiver.resubscribe(), gas_sender)
            .await;
    });

    // start the tx sender
    let (tx_sender, mut tx_receiver) = broadcast::channel(10);
    tokio::spawn(send_transactions(
        signer_provider,
        tx_receiver.resubscribe(),
    ));

    // finally.... start the searcher!!!!!
    tokio::spawn(search_paths(
        graph,
        cycles,
        address_to_pool,
        Arc::new(token_to_edge),
        reserve_update_receiver,
        tx_sender,
    ));

    loop {
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;
    }

    Ok(())
}
