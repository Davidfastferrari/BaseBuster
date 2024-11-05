use log::{debug, info, warn};
use std::collections::HashSet;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;
use alloy::transports::http::{Client, Http};
use alloy::network::Ethereum;
use alloy::providers::{Provider, RootProvider};
use alloy::primitives::{Address, U256};
use alloy::providers::ProviderBuilder;
use pool_sync::PoolType;
use anyhow::{anyhow, Result};
use futures::executor::block_on;

use crate::events::Event;
use crate::market_state::MarketState;
use crate::swap::SwapPath;
use crate::gen::{FlashQuoter, V3State, V2State};
use crate::calculation::Calculator;
use crate::quoter::Quoter;
use crate::AMOUNT;

// recieve a stream of potential arbitrage paths from the searcher and
// simulate them against the contract to determine if they are actually viable
pub async fn simulate_paths(
    tx_sender: Sender<Event>, 
    arb_receiver: Receiver<Event>, 
    market_state: Arc<MarketState<Http<Client>, Ethereum, RootProvider<Http<Client>>>>
) {
    // Construct a new quoter

    // if this is just a sim run or not
    let sim: bool = std::env::var("SIM").unwrap().parse().unwrap();

    // blacklisted paths, some error in swapping that wasnt caught during filter
    let mut blacklisted_paths: HashSet<u64> = HashSet::new();

    // recieve new paths from the searcher
    while let Ok(Event::ArbPath((arb_path, expected_out, block_number))) = arb_receiver.recv() {
        // convert from searcher format into quoter format
        let converted_path: Vec<FlashQuoter::SwapStep> = arb_path.clone().into();

        // get the quote for the path and handle it appropriately
        // if we have not blacklisted the path
        if !blacklisted_paths.contains(&arb_path.hash) {
            // get an initial quote to see if we can swap
            // get read access to the db so we can quote the path
            match Quoter::quote_path(
                    converted_path.clone(), 
                    *AMOUNT, 
                    market_state.clone()
                ) {
                Ok(quote) => {
                    // if we are just simulated, compare to the expected amount
                    if sim {
                        if *(quote.last().unwrap()) == expected_out {
                            info!("Success.. Calculated {expected_out}, Quoted: {}, Path Hash {}", quote.last().unwrap(), arb_path.hash);
                        } else {
                            let calculator = Calculator::new(market_state.clone());
                            let output = calculator.debug_calculation(&arb_path);
                            info!(
                                "Fail.. Calculated {expected_out}, Quoted: {:#?}, Path Hash {}, Path: {:#?} {:#?}",
                                quote,
                                arb_path.hash,
                                converted_path,
                                output
                            );

                            // we need to figure out where we went off sync
                            //block_on(
                                //debug_arb(arb_path.clone(), market_state.clone())
                            //);

                        }
                    } else {
                        // optimize the input amount
                        //info!("Simulation Successful... Calculated {expected_out}, Quoted: {}, Input amount {}, Path: {:#?}", *AMOUNT, converted_path);

                        /*
                        let (optimized_input, optimized_output) =
                            quoter.optimize_input(converted_path.clone()).unwrap();
                        info!(
                            "Optimized input... Optimal amount in {}, Optimized Amount out {}",
                            optimized_input, optimized_output
                        );

                        // send the optimize path to the tx sender
                        let optimized_input = *AMOUNT;
                        match tx_sender.send(Event::ArbPath((arb_path, optimized_input))) {
                            Ok(_) => debug!("Simulator sent path to Tx Sender"),
                            Err(_) => warn!("Simulator: failed to send path to tx sender"),
                        }
                        */
                    }
                }
                Err(quote_err) => {
                    warn!("Failed to simulate quote {}", quote_err);
                    blacklisted_paths.insert(arb_path.hash);
                }
            }
        }
    }
}

pub async fn debug_arb(
    arb_path: SwapPath,
    market_state: Arc<MarketState<Http<Client>, Ethereum, RootProvider<Http<Client>>>>
) -> Result<()> {
    println!("here");
    // Setup provider for chain state
    let provider = Arc::new(ProviderBuilder::new()
        .on_http(std::env::var("FULL").unwrap().parse().unwrap()));
    let block_number = provider.get_block_number().await.unwrap();

    for step in arb_path.steps {
        let pool_address = step.pool_address;
        
        // Get pool type from market state
        let read = market_state.db.read().unwrap();
        let pool_info = read.pool_info.get(&pool_address)
            .ok_or_else(|| anyhow!("Pool not found in database"))?;

        match pool_info.pool_type {
            PoolType::UniswapV2 | PoolType::SushiSwapV2 | PoolType::PancakeSwapV2 
            | PoolType::BaseSwapV2 | PoolType::SwapBasedV2 | PoolType::DackieSwapV2 
            | PoolType::AlienBaseV2 => {
                // Get V2 state from DB
                let (db_reserve0, db_reserve1) = market_state.db.read().unwrap()
                    .get_reserves(&pool_address);

                // get the chain state
                let V2State::getReservesReturn {
                    reserve0: res0,
                    reserve1: res1,
                    ..
                } = V2State::new(pool_address, provider.clone())
                    .getReserves()
                    .call()
                    .await
                    .unwrap();

                // compare the values
                assert_eq!(U256::from(res0), db_reserve0, "reserve 0 mismatch, (chain, db)");
                assert_eq!(U256::from(res1), db_reserve1, "reserve 1 mismatch, (chain, db)");
            },

            PoolType::UniswapV3 | PoolType::PancakeSwapV3 | PoolType::BaseSwapV3 
            | PoolType::SwapBasedV3 | PoolType::AlienBaseV3 | PoolType::DackieSwapV3 => {
                let contract = V3State::new(pool_address, provider.clone());

                // Get V3 state from DB
                let db_slot0 = market_state.db.read().unwrap().slot0(pool_address)?;
                let db_liquidity = market_state.db.read().unwrap().liquidity(pool_address)?;

                    // Get slot0 data
                let V3State::slot0Return { 
                    sqrtPriceX96,
                    tick,
                    ..
                } = contract
                    .slot0()
                    .block(block_number.into())
                    .call()
                    .await
                    .unwrap();

                // Get liquidity
                let V3State::liquidityReturn { _0: liquidity } = contract
                    .liquidity()
                    .block(block_number.into())
                    .call()
                    .await
                    .unwrap();

                    /*j 
                // Get tick data for all initialized ticks
                for (tick, tick_info) in &v3_pool.ticks {
                    let V3State::ticksReturn { 
                        liquidityGross,
                        liquidityNet,
                        ..
                    } = contract
                        .ticks((*tick).try_into().unwrap())
                        .block(last_synced_block.into())
                        .call()
                        .await
                        .unwrap();

                    assert_eq!(tick_info.liquidity_gross, liquidityGross as u128, "Liquidity Gross at tick {}: Address {}, Pool Type {}", tick, pool.address(), pool.pool_type());
                    assert_eq!(tick_info.liquidity_net, liquidityNet as i128, "Liquidity Net at tick {}: Address {}, Pool Type {}", tick, pool.address(), pool.pool_type());
                }
                */

                println!("running here {}", pool_address);
                // Assert all values match
                assert_eq!(db_slot0.sqrtPriceX96, sqrtPriceX96, "SqrtPrice: Address {}, Pool Type {}", pool_address, pool_info.pool_type);
                assert_eq!(db_slot0.tick, tick, "Tick: Address {}, Pool Type {}", pool_address, pool_info.pool_type);
                assert_eq!(db_liquidity, liquidity as u128, "Liquidity: Address {}, Pool Type {}", pool_address, pool_info.pool_type);

            },
            _ => warn!("Unsupported pool type for comparison")
        }
    }

    Ok(())
}
