use alloy::primitives::Address;
use alloy::providers::{Provider, ProviderBuilder, WsConnect};
use alloy::rpc::types::Filter;
use alloy::rpc::types::Log;
use alloy::sol;
use alloy_sol_types::SolEvent;
use futures::stream::StreamExt;
use log::{debug, info};
use pool_sync::pools::pool_structure::{UniswapV2Pool, UniswapV3Pool};
use pool_sync::{Pool, PoolInfo};
use rustc_hash::{FxHashMap, FxHashSet};
use std::sync::{Arc, RwLock};
use tokio::sync::broadcast;

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
    address_to_v2pool: RwLock<FxHashMap<Address, UniswapV2Pool>>,
    /// Mapping from address to V3Pool
    address_to_v3pool: RwLock<FxHashMap<Address, UniswapV3Pool>>,
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
                let v2_pool: UniswapV2Pool = pool.get_v2().unwrap().clone();
                address_to_v2pool.insert(pool.address(), v2_pool);
            } else if pool.is_v3() {
                let v3_pool: UniswapV3Pool = pool.get_v3().unwrap().clone();
                address_to_v3pool.insert(pool.address(), v3_pool);
            }
        }

        let manager = Arc::new(Self {
            addresses,
            address_to_pool,
            address_to_v2pool: RwLock::new(address_to_v2pool),
            address_to_v3pool: RwLock::new(address_to_v3pool),
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
                    let mut map = self.address_to_v3pool.write().unwrap();
                    let pool = map.get_mut(&address).unwrap();
                    pool_sync::pools::process_tick_data(pool, log);
                } else if pool.is_v2() {
                    let mut map = self.address_to_v2pool.write().unwrap();
                    let pool = map.get_mut(&address).unwrap();
                    pool_sync::pools::process_sync_data(pool, log);
                }
            }
        }
        updated_pools
    }

    pub fn get_pool(&self, address: &Address) -> Pool {
        self.address_to_pool.get(address).unwrap().clone()
    }

    pub fn get_v2pool(&self, address: &Address) -> UniswapV2Pool {
        self.address_to_v2pool
            .read()
            .unwrap()
            .get(address)
            .unwrap()
            .clone()
    }

    pub fn get_v3pool(&self, address: &Address) -> UniswapV3Pool {
        self.address_to_v3pool
            .read()
            .unwrap()
            .get(address)
            .unwrap()
            .clone()
    }

    pub fn zero_to_one(&self, token_in: Address, pool: &Address) -> bool {
        let pool = self.address_to_pool.get(pool).unwrap();
        token_in == pool.token0_address()
    }
}
