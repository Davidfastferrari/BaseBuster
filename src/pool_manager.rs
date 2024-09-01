use alloy::primitives::{Address, U128, U256};
use alloy::providers::{Provider, ProviderBuilder, WsConnect};
use alloy::rpc::types::Filter;
use alloy::rpc::types::Log;
use alloy::sol;
use alloy::sol_types::SolEvent;
use futures::stream::StreamExt;
use log::{debug, info};
use pool_sync::{
    BalancerV2Pool, MaverickPool, TickInfo, UniswapV2Pool, UniswapV3Pool,
    CurveTriCryptoPool, CurveTwoCryptoPool
};
use pool_sync::{Pool, PoolInfo, PoolType};
use rustc_hash::{FxHashMap, FxHashSet};
use std::collections::HashSet;
use std::sync::RwLockReadGuard;
use std::sync::{Arc, RwLock};
use tokio::sync::broadcast;

use crate::events::Event;

sol!(
    #[derive(Debug)]
    contract AerodromeEvent {
        event Sync(uint256 reserve0, uint256 reserve1);
    }
);

sol!(
    #[derive(Debug)]
    #[sol(rpc)]
    contract PancakeSwap {
        event Swap(
            address indexed sender,
            address indexed recipient,
            int256 amount0,
            int256 amount1,
            uint160 sqrtPriceX96,
            uint128 liquidity,
            int24 tick,
            uint128 protocolFeesToken0,
            uint128 protocolFeesToken1
        );
    }
);
sol! {
    #[derive(Debug)]
    contract BalancerV2Event {
        event PoolBalanceChanged(
            bytes32 indexed poolId,
            address indexed liquidityProvider,
            address[] tokens,
            int256[] deltas,
            uint256[] protocolFeeAmounts
        );
    }
}


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
    /// Mapping from address to Balancer Pool
    address_to_balancerpool: FxHashMap<Address, RwLock<BalancerV2Pool>>,
    /// Mapping from address to CurveTwoCryptoPool
    address_to_curvetwopool: FxHashMap<Address, CurveTwoCryptoPool>,
    /// Mapping from address to CurveTriCryptoPool
    address_to_curvetripool: FxHashMap<Address, CurveTriCryptoPool>,
}

impl PoolManager {
    // construct a new instance
    pub async fn new(
        working_pools: Vec<Pool>,
        sender: broadcast::Sender<Event>,
        last_synced_block: u64,
    ) -> Arc<Self> {
        // if a pool is v3, make sure it has liuqidity
        let filtered_pools: Vec<Pool> = working_pools.into_iter().filter(|pool| {
            if pool.is_v3() {
                let v3_pool = pool.get_v3().unwrap();
                return v3_pool.liquidity > 0;
            };
            // keep all other pools
            true
        }).collect();

        let address_to_pool: FxHashMap<Address, Pool> = filtered_pools
            .iter()
            .map(|pool| (pool.address(), pool.clone()))
            .collect();

        let addresses = address_to_pool.keys().cloned().collect();

        let mut address_to_v2pool = FxHashMap::default();
        let mut address_to_v3pool = FxHashMap::default();
        let mut address_to_balancerpool = FxHashMap::default();
        let mut address_to_curvetwopool = FxHashMap::default();
        let mut address_to_curvetripool = FxHashMap::default();
        for pool in filtered_pools {
            if pool.is_v2() {
                let v2_pool = pool.get_v2().unwrap().clone();
                address_to_v2pool.insert(pool.address(), RwLock::new(v2_pool));
            } else if pool.is_v3() {
                let v3_pool = pool.get_v3().unwrap().clone();
                address_to_v3pool.insert(pool.address(), RwLock::new(v3_pool));
            } else if pool.is_balancer() {
                let balancer_pool = pool.get_balancer().unwrap().clone();
                address_to_balancerpool.insert(pool.address(), RwLock::new(balancer_pool));
            } else if pool.is_curve_two() {
                let curve_pool = pool.get_curve_two().unwrap().clone();
                address_to_curvetwopool.insert(pool.address(), curve_pool);
            } else if pool.is_curve_tri() {
                let curve_pool = pool.get_curve_tri().unwrap().clone();
                address_to_curvetripool.insert(pool.address(), curve_pool);
            }
        }

        let manager = Arc::new(Self {
            addresses,
            address_to_pool,
            address_to_v2pool,
            address_to_v3pool,
            address_to_balancerpool,
            address_to_curvetwopool,
            address_to_curvetripool
        });

        tokio::spawn(PoolManager::state_updater(
            manager.clone(),
            sender,
            last_synced_block,
        ));
        manager
    }


    /// Updater thread that will process the logs after each block and update the corresponding reserves
    pub async fn state_updater(
        manager: Arc<PoolManager>,
        sender: broadcast::Sender<Event>,
        mut last_synced_block: u64,
    ) {
        let ws_url = std::env::var("WS").unwrap();
        let http_url = std::env::var("FULL").unwrap();

        let ws = ProviderBuilder::new()
            .on_ws(WsConnect::new(ws_url))
            .await
            .unwrap();
        let http = ProviderBuilder::new().on_http(http_url.parse().unwrap());

        // process the missed blocks
        let mut latest_block = http.get_block_number().await.unwrap();
        while last_synced_block < latest_block {
            let filter = Filter::new()
                .events([
                    BalancerV2Event::PoolBalanceChanged::SIGNATURE,
                    PancakeSwap::Swap::SIGNATURE,
                    AerodromeEvent::Sync::SIGNATURE,
                    DataEvent::Sync::SIGNATURE,
                    DataEvent::Mint::SIGNATURE,
                    DataEvent::Burn::SIGNATURE,
                    DataEvent::Swap::SIGNATURE,
                ])
                .from_block(last_synced_block + 1)
                .to_block(latest_block);
            let logs = http.get_logs(&filter).await.unwrap();
            let _ = manager.process_logs(logs);
            last_synced_block = latest_block;
            latest_block = http.get_block_number().await.unwrap();
        }

        // start block stream to continuously process blocks
        let sub = ws.subscribe_blocks().await.unwrap();
        let mut stream = sub.into_stream();
        while let Some(block) = stream.next().await {
            println!("New block: {:?}", block.header.number);
            let block_number = block.header.number;

            // setup the log filters
            let filter = Filter::new()
                .events([
                    BalancerV2Event::PoolBalanceChanged::SIGNATURE,
                    PancakeSwap::Swap::SIGNATURE,
                    AerodromeEvent::Sync::SIGNATURE,
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


    // process the logs in a block
    fn process_logs(&self, logs: Vec<Log>) -> Vec<Address> {
        let mut updated_pools = HashSet::new();
        for log in logs {
            let address = log.address();
            // we know if it s v3 pool since we are processing mint/burn/swap logs
            if self.addresses.contains(&address) {
                updated_pools.insert(address);
                let pool = self.get_pool(&address);
                let pool_type = pool.pool_type();
                if pool.is_v3() {
                    if let Some(pool_lock) = self.address_to_v3pool.get(&address) {
                        let mut pool = pool_lock.write().unwrap();
                        process_tick_data(&mut pool, log, pool_type);
                    }
                } else if pool.is_v2() {
                    if let Some(pool_lock) = self.address_to_v2pool.get(&pool.address()) {
                        let mut pool = pool_lock.write().unwrap();
                        process_sync_data(&mut pool, log, pool_type);
                    }
                } else if pool.is_balancer() {
                    if let Some(pool_lock) = self.address_to_balancerpool.get(&pool.address()) {
                        let mut pool = pool_lock.write().unwrap();
                        process_balance_data(&mut pool, log);
                    }
                }
            }
        }
        updated_pools.into_iter().collect()
    }



    // METHODS TO RETRIEVE SPECIFIC POOLS FROM THE MANAGER
    // ------------------------------------------------
    pub fn get_pool(&self, address: &Address) -> Pool {
        self.address_to_pool.get(address).unwrap().clone()
    }

    pub fn get_v2pool(&self, address: &Address) -> RwLockReadGuard<UniswapV2Pool> {
        self.address_to_v2pool.get(address).unwrap().read().unwrap()
    }

    pub fn get_v3pool(&self, address: &Address) -> RwLockReadGuard<UniswapV3Pool> {
        self.address_to_v3pool.get(address).unwrap().read().unwrap()
    }

    pub fn get_balancer_pool(&self, address: &Address) -> RwLockReadGuard<BalancerV2Pool> {
        self.address_to_balancerpool.get(address).unwrap().read().unwrap()
    }

    pub fn get_curve_two_pool(&self, address: &Address) -> &CurveTwoCryptoPool {
        self.address_to_curvetwopool.get(address).unwrap()
    }

    pub fn get_curve_tri_pool(&self, address: &Address) -> &CurveTriCryptoPool {
        self.address_to_curvetripool.get(address).unwrap()
    }



    pub fn zero_to_one(&self, token_in: Address, pool: &Address) -> bool {
        let pool = self.address_to_pool.get(pool).unwrap();
        token_in == pool.token0_address()
    }
}


fn process_balance_data(pool: &mut BalancerV2Pool, log: Log) {
    let event = BalancerV2Event::PoolBalanceChanged::decode_log(log.as_ref(), true).unwrap();
    println!("got new balancer event");
    

    for (token, delta) in event.tokens.iter().zip(event.deltas.iter()) {
        if let Some(index) = pool.get_token_index(token) {
            // Update the balance for the token
            let delta_abs = delta.abs().try_into().unwrap_or(U256::MAX);
            if delta.is_negative() {
                pool.balances[index] = pool.balances[index].saturating_sub(delta_abs);
            } else {
                pool.balances[index] = pool.balances[index].saturating_add(delta_abs);
            }
        }
    }
}

pub fn process_tick_data(pool: &mut UniswapV3Pool, log: Log, pool_type: PoolType) {
    let event_sig = log.topic0().unwrap();
    //println!("Before");
    //println!("address {}, liquidity {}, tick {}, sqrt_price {}", pool.address, pool.liquidity, pool.tick, pool.sqrt_price);


    if *event_sig == DataEvent::Burn::SIGNATURE_HASH {
        process_burn(pool, log);
    } else if *event_sig == DataEvent::Mint::SIGNATURE_HASH {
        process_mint(pool, log);
    } else if *event_sig == DataEvent::Swap::SIGNATURE_HASH || *event_sig == PancakeSwap::Swap::SIGNATURE_HASH {
        process_swap(pool, log, pool_type);
    }
    //println!("After");
    //println!("address {}, liquidity {}, tick {}, sqrt_price {}", pool.address, pool.liquidity, pool.tick, pool.sqrt_price);
    //println!("After processing: liquidity = {}", pool.liquidity);

}

fn process_burn(pool: &mut UniswapV3Pool, log: Log) {
    let burn_event = DataEvent::Burn::decode_log(log.as_ref(), true).unwrap();
    modify_position(
        pool,
        burn_event.tickLower.as_i32(),
        burn_event.tickUpper.as_i32(),
        -(burn_event.amount as i128),
    );
}

fn process_mint(pool: &mut UniswapV3Pool, log: Log) {
    let mint_event = DataEvent::Mint::decode_log(log.as_ref(), true).unwrap();
    modify_position(
        pool,
        mint_event.tickLower.as_i32(),
        mint_event.tickUpper.as_i32(),
        mint_event.amount as i128,
    );
}

fn process_swap(pool: &mut UniswapV3Pool, log: Log, pool_type: PoolType) {
    if pool_type == PoolType::PancakeSwapV3{
        let swap_event = PancakeSwap::Swap::decode_log(log.as_ref(), true).unwrap();
        pool.tick = swap_event.tick.as_i32();
        pool.sqrt_price = U256::from(swap_event.sqrtPriceX96);
        pool.liquidity = swap_event.liquidity;
    } else {
        let swap_event = DataEvent::Swap::decode_log(log.as_ref(), true).unwrap();
        pool.tick = swap_event.tick.as_i32();
        pool.sqrt_price = U256::from(swap_event.sqrtPriceX96);
        pool.liquidity = swap_event.liquidity;
    }
}

/// Modifies a positions liquidity in the pool.
pub fn modify_position(
    pool: &mut UniswapV3Pool,
    tick_lower: i32,
    tick_upper: i32,
    liquidity_delta: i128,
) {
    //We are only using this function when a mint or burn event is emitted,
    //therefore we do not need to checkTicks as that has happened before the event is emitted
    update_position(pool, tick_lower, tick_upper, liquidity_delta);

    if liquidity_delta != 0 {
        //if the tick is between the tick lower and tick upper, update the liquidity between the ticks
        if pool.tick >= tick_lower && pool.tick < tick_upper {
            if liquidity_delta < 0 {
                pool.liquidity = pool.liquidity.checked_sub((-liquidity_delta) as u128).unwrap_or(0);
            } else {
                pool.liquidity = pool.liquidity.checked_add(liquidity_delta as u128).unwrap_or(u128::MAX);
            }
        }
    }
}

pub fn update_position(
    pool: &mut UniswapV3Pool,
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
    pool: &mut UniswapV3Pool,
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

pub fn flip_tick(pool: &mut UniswapV3Pool, tick: i32, tick_spacing: i32) {
    let (word_pos, bit_pos) = uniswap_v3_math::tick_bitmap::position(tick / tick_spacing);
    let mask = U256::from(1) << bit_pos;

    if let Some(word) = pool.tick_bitmap.get_mut(&word_pos) {
        *word ^= mask;
    } else {
        pool.tick_bitmap.insert(word_pos, mask);
    }
}

pub fn process_sync_data(pool: &mut UniswapV2Pool, log: Log, pool_type: PoolType) {
    if pool_type == PoolType::Aerodrome {
        let sync_event = AerodromeEvent::Sync::decode_log(log.as_ref(), true).unwrap();
        pool.token0_reserves = U128::from(sync_event.reserve0);
        pool.token1_reserves = U128::from(sync_event.reserve1);
    } else {
        let sync_event = DataEvent::Sync::decode_log(log.as_ref(), true).unwrap();
        pool.token0_reserves = U128::from(sync_event.reserve0);
        pool.token1_reserves = U128::from(sync_event.reserve1);
    }
}
