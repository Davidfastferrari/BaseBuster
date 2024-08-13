use alloy::primitives::{Address, U128, U256};
use alloy::providers::{Provider, ProviderBuilder, WsConnect};
use alloy::rpc::types::Filter;
use alloy::rpc::types::Log;
use alloy::sol;
use alloy_sol_types::SolEvent;
use futures::stream::StreamExt;
use log::{debug, info};
use pool_sync::pools::pool_structure::{TickInfo, UniswapV2Pool, UniswapV3Pool};
use pool_sync::{Pool, PoolInfo};
use rustc_hash::{FxHashMap, FxHashSet};
use tokio::sync::broadcast;
use std::sync::{Arc, RwLock, RwLockWriteGuard};
use std::sync::RwLockReadGuard;

use crate::events::Event;

sol! {
    #[derive(Debug)]
    contract DataEvent {
        event Sync(uint112 reserve0, uint112 reserve1);
        event Swap(
            address indexed sender,
            address indexed recipient,
            int256 amount0,
            int256 amount1,
            uint160 sqrtPriceX96,
            uint128 liquidity,
            int24 tick
        );

        event Mint(
            address sender,
            address indexed owner,
            int24 indexed tickLower,
            int24 indexed tickUpper,
            uint128 amount,
            uint256 amount0,
            uint256 amount1
        );

        event Burn(
            address indexed owner,
            int24 indexed tickLower,
            int24 indexed tickUpper,
            uint128 amount,
            uint256 amount0,
            uint256 amount1
        );
    }
}
// Structure to hold all the tracked pools
// Reserves will be modified on every block due to Sync events
#[derive(Default)]
pub struct PoolManager {
    // All addresses that hte pool is tracking
    addresses: FxHashSet<Address>,
    // Mapping from address to generic pool
    address_to_pool: FxHashMap<Address, Pool>,
    // Mapping from address to V2Pool
    address_to_v2pool: FxHashMap<Address, RwLock<UniswapV2Pool>>,
    /// Mapping from address to V3Pool
    address_to_v3pool: FxHashMap<Address, RwLock<UniswapV3Pool>>,
}

impl PoolManager {
    // construct a new instance
    pub async fn new(working_pools: Vec<Pool>, sender: broadcast::Sender<Event>) -> Arc<Self> {
        let address_to_pool: FxHashMap<Address, Pool> = working_pools
            .iter()
            .map(|pool| (pool.address(), pool.clone()))
            .collect();

        let addresses = address_to_pool.keys().cloned().collect();

        let mut address_to_v2pool = FxHashMap::default();
        let mut address_to_v3pool = FxHashMap::default();
        for pool in working_pools {
            if pool.is_v2() {
                let v2_pool = pool.get_v2().unwrap().clone();
                address_to_v2pool.insert(pool.address(), RwLock::new(v2_pool));

            } else if pool.is_v3() {
                let v3_pool = pool.get_v3().unwrap().clone();
                address_to_v3pool.insert(pool.address(), RwLock::new(v3_pool));
            }
        }

        let manager = Arc::new(Self {
            addresses,
            address_to_pool,
            address_to_v2pool,
            address_to_v3pool,
        });

        tokio::spawn(PoolManager::state_updater(manager.clone(), sender));
        manager
    }

    pub async fn state_updater(manager: Arc<PoolManager>, sender: broadcast::Sender<Event>) {
        let ws_url = std::env::var("WS").unwrap();
        let http_url = std::env::var("FULL").unwrap();

        let ws = ProviderBuilder::new()
            .on_ws(WsConnect::new(ws_url))
            .await
            .unwrap();
        let http = ProviderBuilder::new().on_http(http_url.parse().unwrap());

        let sub = ws.subscribe_blocks().await.unwrap();
        let mut stream = sub.into_stream();
        while let Some(block) = stream.next().await {
            let block_number = block.header.number.unwrap();

            // setup the log filters
            let filter = Filter::new()
                .events([
                    DataEvent::Sync::SIGNATURE,
                    DataEvent::Mint::SIGNATURE,
                    DataEvent::Burn::SIGNATURE,
                    DataEvent::Swap::SIGNATURE,
                ])
                .from_block(block_number);

            let logs = http.get_logs(&filter).await.unwrap();

            let updated_pools = manager.process_logs(logs);
            match sender.send(Event::ReserveUpdate(updated_pools)) {
                Ok(_) => debug!("Reserves updated"),
                Err(e) => info!("Reserves update failed: {:?}", e),
            }
        }
    }

    fn process_logs(&self, logs: Vec<Log>) -> Vec<Address> {
        let mut updated_pools = Vec::new();
        for log in logs {
            let address = log.address();
            // we know if it s v3 pool since we are processing mint/burn/swap logs
            if self.addresses.contains(&address) {
                updated_pools.push(address);
                let pool = self.get_pool(&address);
                if pool.is_v3() {
                    let mut pool = self.address_to_v3pool.get(&pool.address()).unwrap().write().unwrap();
                    process_tick_data(&mut pool, log);
                } else if pool.is_v2() {
                    let mut pool = self.address_to_v2pool.get(&pool.address()).unwrap().write().unwrap();
                    process_sync_data(&mut pool, log);
                }
            }
        }
        updated_pools
    }

    pub fn get_pool(&self, address: &Address) -> Pool {
        self.address_to_pool.get(address).unwrap().clone()
    }

    pub fn get_v2pool(&self, address: &Address) -> RwLockReadGuard<UniswapV2Pool> {
        self.address_to_v2pool.get(address).unwrap().read().unwrap()
    }

    pub fn get_v3pool(&self, address: &Address) -> RwLockReadGuard<UniswapV3Pool> {
        self.address_to_v3pool.get(address).unwrap().read().unwrap()
    }

    pub fn zero_to_one(&self, token_in: Address, pool: &Address) -> bool {
        let pool = self.address_to_pool.get(pool).unwrap();
        token_in == pool.token0_address()
    }
}


pub fn process_tick_data(pool: &mut RwLockWriteGuard<UniswapV3Pool>, log: Log) {
    let event_sig = log.topic0().unwrap();

    if *event_sig == DataEvent::Burn::SIGNATURE_HASH {
        process_burn(pool, log);
    } else if *event_sig == DataEvent::Mint::SIGNATURE_HASH {
        process_mint(pool, log);
    } else if *event_sig == DataEvent::Swap::SIGNATURE_HASH {
        process_swap(pool, log);
    }
}

fn process_burn(pool: &mut RwLockWriteGuard<UniswapV3Pool>, log: Log) {
    let burn_event = DataEvent::Burn::decode_log(log.as_ref(), true).unwrap();
    modify_position(
        pool,
        burn_event.tickLower,
        burn_event.tickUpper,
        -(burn_event.amount as i128)
    );
}

fn process_mint(pool: &mut RwLockWriteGuard<UniswapV3Pool>, log: Log) {
    let mint_event = DataEvent::Mint::decode_log(log.as_ref(), true).unwrap();
    modify_position(
        pool,
        mint_event.tickLower,
        mint_event.tickUpper,
        mint_event.amount as i128
    );
}

fn process_swap(pool: &mut RwLockWriteGuard<UniswapV3Pool>, log: Log) {
    let swap_event = DataEvent::Swap::decode_log(log.as_ref(), true).unwrap();
    pool.tick = swap_event.tick;
    pool.sqrt_price = swap_event.sqrtPriceX96;
    pool.liquidity = swap_event.liquidity;
}

/// Modifies a positions liquidity in the pool.
pub fn modify_position(
    pool: &mut RwLockWriteGuard<UniswapV3Pool>,
    tick_lower: i32,
    tick_upper: i32,
    liquidity_delta: i128,
) {
    //We are only using this function when a mint or burn event is emitted,
    //therefore we do not need to checkTicks as that has happened before the event is emitted
    update_position(pool, tick_lower, tick_upper, liquidity_delta);

    if liquidity_delta != 0 {
        //if the tick is between the tick lower and tick upper, update the liquidity between the ticks
        if pool.tick > tick_lower && pool.tick < tick_upper {
            pool.liquidity = if liquidity_delta < 0 {
                pool.liquidity - ((-liquidity_delta) as u128)
            } else {
                pool.liquidity + (liquidity_delta as u128)
            }
        }
    }
}

pub fn update_position(
    pool: &mut RwLockWriteGuard<UniswapV3Pool>,
    tick_lower: i32,
    tick_upper: i32,
    liquidity_delta: i128,
) {
    let mut flipped_lower = false;
    let mut flipped_upper = false;

    if liquidity_delta != 0 {
        flipped_lower = update_tick(pool, tick_lower, liquidity_delta, false);
        flipped_upper = update_tick(pool, tick_upper, liquidity_delta, true);
        if flipped_lower {
            flip_tick(pool, tick_lower, pool.tick_spacing);
        }
        if flipped_upper {
            flip_tick(pool, tick_upper, pool.tick_spacing);
        }
    }

    if liquidity_delta < 0 {
        if flipped_lower {
            pool.ticks.remove(&tick_lower);
        }

        if flipped_upper {
            pool.ticks.remove(&tick_upper);
        }
    }
}

pub fn update_tick(
    pool: &mut RwLockWriteGuard<UniswapV3Pool>,
    tick: i32,
    liquidity_delta: i128,
    upper: bool,
) -> bool {
    let info = match pool.ticks.get_mut(&tick) {
        Some(info) => info,
        None => {
            pool.ticks.insert(tick, TickInfo::default());
            pool.ticks
                .get_mut(&tick)
                .expect("Tick does not exist in ticks")
        }
    };

    let liquidity_gross_before = info.liquidity_gross;

    let liquidity_gross_after = if liquidity_delta < 0 {
        liquidity_gross_before - ((-liquidity_delta) as u128)
    } else {
        liquidity_gross_before + (liquidity_delta as u128)
    };

    // we do not need to check if liqudity_gross_after > maxLiquidity because we are only calling update tick on a burn or mint log.
    // this should already be validated when a log is
    let flipped = (liquidity_gross_after == 0) != (liquidity_gross_before == 0);

    if liquidity_gross_before == 0 {
        info.initialized = true;
    }

    info.liquidity_gross = liquidity_gross_after;

    info.liquidity_net = if upper {
        info.liquidity_net - liquidity_delta
    } else {
        info.liquidity_net + liquidity_delta
    };

    flipped
}

pub fn flip_tick(pool: &mut RwLockWriteGuard<UniswapV3Pool>, tick: i32, tick_spacing: i32) {
    let (word_pos, bit_pos) = uniswap_v3_math::tick_bitmap::position(tick / tick_spacing);
    let mask = U256::from(1) << bit_pos;

    if let Some(word) = pool.tick_bitmap.get_mut(&word_pos) {
        *word ^= mask;
    } else {
        pool.tick_bitmap.insert(word_pos, mask);
    }
}

pub fn process_sync_data(pool: &mut RwLockWriteGuard<UniswapV2Pool>, log: Log) {
    let sync_event = DataEvent::Sync::decode_log(log.as_ref(), true).unwrap();
    pool.token0_reserves = U128::from(sync_event.reserve0);
    pool.token1_reserves = U128::from(sync_event.reserve1);
}