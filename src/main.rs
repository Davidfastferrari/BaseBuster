

use crate::price_graph::PriceGraph;
use crate::edge::*;
use std::sync::Arc;

mod price_graph;
mod path;
mod edge;
mod types;


fn main() {
    let mut graph = PriceGraph::new();
    graph.add_edge(0, 1, 1, 30, Arc::new(UniswapV2Edge { 
        reserve_in: 1_000_000, 
        reserve_out: 1_000_000, 
        fee: 30, 
        exchange: 1 
    }));
    graph.add_edge(0, 1, 2, 30, Arc::new(UniswapV2Edge { 
        reserve_in: 990_000, 
        reserve_out: 1_010_000, 
        fee: 30, 
        exchange: 2 
    }));
    graph.add_edge(1, 2, 1, 500, Arc::new(UniswapV3Edge { 
        sqrt_price_x96: 1u128 << 96, 
        liquidity: 1_000_000, 
        fee: 500, 
        exchange: 1,
        zero_for_one: true 
    }));
    graph.add_edge(2, 0, 1, 30, Arc::new(UniswapV2Edge { 
        reserve_in: 1_000_000, 
        reserve_out: 990_000, 
        fee: 30, 
        exchange: 1 
    }));

    graph.set_block_number();



}








































































/*
use crate::types::Pair;
use crate::types::*;
use alloy::primitives::{address, Address};
use alloy::providers::{Provider, ProviderBuilder, RootProvider, WsConnect};
use anyhow::Result;
use log::{debug, info, LevelFilter};
use pool_sync::{Pool, PoolInfo, PoolSync, PoolType};
use price_graph::*;
use std::sync::Arc;

mod constants;
mod path;
mod price_graph;
mod types;

#[tokio::main]
async fn main() -> Result<()> {
    // logger and env config
    dotenv::dotenv().ok();
    env_logger::builder()
        .filter_level(LevelFilter::Info)
        .build();

    info!("Welcome to base buster");

    debug!("Starting providers");
    // construct the providers
    let ws = WsConnect::new(std::env::var("WSS_URL")?);
    let ws_provider = Arc::new(
        ProviderBuilder::new()
            .network::<alloy::network::AnyNetwork>()
            .on_ws(ws)
            .await?,
    );
    let http_provider = Arc::new(
        ProviderBuilder::new()
            .network::<alloy::network::AnyNetwork>()
            .on_http(std::env::var("HTTP_URL")?.parse()?),
    );
    debug!("Providers started sucessfully");

    // sync the pools
    /*
    let pool_sync = PoolSync::builder()
        .add_pool(PoolType::UniswapV2)
        .rate_limit(10)
        .build()?;
    let pools = pool_sync.sync_pools(http_provider.clone()).await;
    */

    let pairs = load_pairs();
    let pairs: Vec<Pair> = pairs.iter().map(|(p, _)| *p).collect(); 
                                                                               // create path graph
    PriceGraph::find_paths(Token::WETH, pairs.as_slice());
    //

    Ok(())
}

// load all the pairs in, mapping of the pair to its facotyr address, need to migrate to pools
fn load_pairs() -> Vec<(Pair, Address)> {
    let pairs: &[(Pair, Address)] = &[
        (
            Pair::new(Token::WETH, Token::USDC, 300, ExchangeId::UniswapV2),
            address!("88A43bbDF9D098eEC7bCEda4e2494615dfD9bB9C"),
        ),
        (
            Pair::new(Token::WETH, Token::DAI, 300, ExchangeId::UniswapV2),
            address!("b2839134B8151964f19f6f3c7D59C70ae52852F5"),
        ),
        (
            Pair::new(Token::DAI, Token::USDC, 300, ExchangeId::Basescan),
            address!("8b7cc11ff640a494e5a31a82415bb0d831e62363"),
        ),
        (
            Pair::new(Token::WETH, Token::BRETT, 300, ExchangeId::UniswapV3),
            address!("76bf0abd20f1e0155ce40a62615a90a709a6c3d8 "),
        ),
        (
            Pair::new(Token::AERO, Token::USDC, 300, ExchangeId::Aerodome),
            address!("6cdcb1c4a4d1c3c6d054b27ac5b77e89eafb971d"),
        ),
        (
            Pair::new(Token::WETH, Token::USDC, 300, ExchangeId::Aerodome),
            address!("b2cc224c1c9fee385f8ad6a55b4d94e92359dc59"),
        ),
    ];
    pairs.to_vec()
}
*/
