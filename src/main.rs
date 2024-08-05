use alloy::network::EthereumWallet;
use alloy::node_bindings::Anvil;
use alloy::primitives::address;
use alloy::providers::{Provider, ProviderBuilder, WsConnect};
use alloy::signers::local::PrivateKeySigner;
use alloy::sol;
use log::{info, LevelFilter};
use pool_sync::*;
use std::sync::Arc;

use crate::graph::ArbGraph;
use crate::ignition::start_workers;
use crate::pool_manager::PoolManager;
use crate::util::get_working_pools;

// Pair contract to get reserves
sol!(
    #[derive(Debug)]
    #[sol(rpc)]
    BatchSync,
    "src/abi/BatchSync.json"
);

sol!(
    #[derive(Debug)]
    #[sol(rpc)]
    FlashSwap,
    "src/abi/FlashSwap.json"
);


mod calculation;
mod events;
mod gas_manager;
mod graph;
mod ignition;
mod market;
mod optimizer;
mod pool_manager;
mod simulation;
mod stream;
mod tx_sender;
mod util;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    // initializations
    dotenv::dotenv().ok();
    env_logger::Builder::new()
        .filter_level(LevelFilter::Info) // or Info, Warn, etc.
        .init();

    // construct the providers
    info!("Constructing http provider...");

    // Http provider, utilizing anvil instance
    let url = std::env::var("FULL").unwrap();
    let http_provider = Arc::new(ProviderBuilder::new().on_http(url.parse().unwrap()));

    // Load in all the pools
    info!("Loading and syncing pools...");
    let pool_sync = PoolSync::builder()
        .add_pools(&[
            PoolType::UniswapV2,
            PoolType::SushiSwapV2,
            PoolType::PancakeSwapV2,
            PoolType::UniswapV3,
            PoolType::SushiSwapV3,
        ])
        .chain(Chain::Ethereum)
        .rate_limit(100)
        .build()
        .unwrap();
    let pools = pool_sync.sync_pools().await.unwrap();

    // start the anvil instance
    let fork_block = http_provider.get_block_number().await.unwrap();
    let anvil = Anvil::new()
        .fork(url)
        .port(8545_u16)
        .fork_block_number(fork_block)
        //.port(portpicker::pick_unused_port().unwrap())
        .try_spawn()
        .unwrap();
    info!("Anvil endpoint: {}", anvil.endpoint_url());

    // create anvil signer and http client on anvil endpoint
    let signer: PrivateKeySigner = anvil.keys()[0].clone().into();
    let wallet = EthereumWallet::from(signer);
    let anvil_signer = Arc::new(
        ProviderBuilder::new()
            .with_recommended_fillers()
            .wallet(wallet)
            .on_http(anvil.endpoint_url()),
    );

    // deploy the batch contract
    let contract = BatchSync::deploy(anvil_signer.clone()).await.unwrap();
    let contract_address = contract.address();

    // deploy the falsh contract
    let flash = FlashSwap::deploy(anvil_signer.clone()).await.unwrap();
    info!("Flash swap address {:#?}", flash.address());
    let flash_address = flash.address();

    // Wallet signers
    let anvil_provider = Arc::new(ProviderBuilder::new().on_http(anvil.endpoint_url()));
    let block = anvil_provider.get_block_number().await.unwrap();
    print!("Block number: {:?}", block);
    // Websocket provider
    let ws_url = WsConnect::new(std::env::var("WS").unwrap());
    let ws_provider = Arc::new(ProviderBuilder::new().on_ws(ws_url).await.unwrap());

    // load in the tokens that have had the top volume
    info!("Getting our set of working pools...");
    let working_pools = get_working_pools(pools, 10000, Chain::Ethereum).await;
    println!("Found {} working pools", working_pools.len());

    // Maintains reserves updates and pool state
    info!("Constructing the pool manager and getting initial reserves...");
    let pool_manager = Arc::new(
        PoolManager::new(
            working_pools.clone(),
            http_provider.clone(),
            contract_address.clone(),
        )
        .await,
    );

    // build the graph and populate mappings
    info!("Constructing graph and generating cycles...");
    let weth = address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");
    let graph = ArbGraph::new(pool_manager.clone(), working_pools.clone(), weth);

    info!("Starting workers...");
    start_workers(
        http_provider,
        anvil_provider,
        ws_provider,
        pool_manager,
        graph,
        *flash_address,
    )
    .await;

    loop {
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
    }

    Ok(())
}
