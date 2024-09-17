use anyhow::Result;
use ignition::start_workers;
use alloy::{primitives::Address, providers::ProviderBuilder};
use alloy::primitives::{address, U256};
use log::{info, LevelFilter};
use pool_sync::*;
use std::collections::BTreeMap;
use alloy::providers::Provider;

use alloy::sol;
use std::time::Instant;

mod graph;
mod calculation;
mod ignition;
mod market;
mod simulator;
mod stream;
mod tx_sender;
mod util;
//mod tests;
mod events;
mod searcher;
mod swap;
mod cache;
mod state_db;
mod tracing;
mod gen;
mod market_state;
mod bytecode;

use dotenv;
use revm::primitives::{ExecutionResult, TransactTo};
use alloy::sol_types::SolCall;
use revm::Evm;
use crate::state_db::BlockStateDB;

// define our flash swap contract
sol!(
    #[derive(Debug)]
    #[sol(rpc)]
    FlashSwap,
    "src/abi/FlashSwap.json"
);

// initial amount we are trying to arb over
pub const AMOUNT: u128 = 10000000000000000;

//#[tokio::main]
fn main() -> Result<()> {
    // init dots and logger
    dotenv::dotenv().ok();
    sol!(
        #[sol(rpc)]
        contract Uniswap {
            function getAmountsOut(uint amountIn, address[] memory path) external view returns (uint[] memory amounts);
        }
    );

    dotenv::dotenv().ok();
    let url = std::env::var("FULL").unwrap().parse().unwrap();
    let provider = ProviderBuilder::new().on_http(url);
    let mut db = BlockStateDB::new(provider.clone());

    let pool_addr = address!("B4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc");
    let token0 = address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"); // USDC
    let token1 = address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"); // WETH

    let amount_in = U256::from(1000000000); // 1 USDC (6 decimals)
    let calldata = Uniswap::getAmountsOutCall {
        amountIn: amount_in,
        path: vec![token0, token1],
    }.abi_encode();

    // Prepare calldata for getAmountsOut

    // Create EVM instance
    let mut evm = Evm::builder()
        .with_db(db)
        .modify_tx_env(|tx| {
            tx.caller = address!("0000000000000000000000000000000000000001");
            tx.transact_to = TransactTo::Call(address!("7a250d5630B4cF539739dF2C5dAcb4c659F2488D"));
            tx.data = calldata.into();
            tx.value = U256::ZERO;
        }).build();

    
    let ref_tx = evm.transact().unwrap();
    println!("{:?}", ref_tx);
    let result = ref_tx.result; 

    println!("{:?}", result);
    Ok(())

    /* 
    env_logger::Builder::new()
        .filter_level(LevelFilter::Info) // or Info, Warn, etc.
        .init();

    let provider = ProviderBuilder::new().on_http(std::env::var("ARCHIVE")?.parse()?);
    let address = address!("B4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc");
    for i in 0..25 {
        let res = provider.get_storage_at(address, U256::from(i)).await?;
        println!("{:?}", res);

    }


    Ok(())

    // Load in all the pools
    info!("Loading and syncing pools...");
    let pool_sync = PoolSync::builder()
        .add_pools(&[PoolType::UniswapV2])
        .chain(Chain::Ethereum)
        .build()?;

    let (pools, last_synced_block) = pool_sync.sync_pools().await?;

    start_workers(pools, last_synced_block).await;
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(1000)).await;
    }
    */
    
}
