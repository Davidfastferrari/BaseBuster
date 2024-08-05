use alloy::eips::eip6110::MAINNET_DEPOSIT_CONTRACT_ADDRESS;
use alloy::primitives::address;
use alloy::primitives::Address;
use alloy::primitives::{U128, U256};
use alloy::providers::RootProvider;
use log::info;
use alloy::sol;
use alloy::transports::http::{Client, Http};
use futures::future::join_all;
use futures::stream::{self, StreamExt};
use log::debug;
use petgraph::prelude::*;
use pool_sync::PoolInfo;
use pool_sync::{Pool, PoolType};
use rustc_hash::FxHashMap;
use rustc_hash::FxHashSet;
use std::sync::Arc;
use std::sync::RwLock;
use tokio::sync::Semaphore;
use pool_sync::PoolSync;
use pool_sync::snapshot::*;


// Structure to hold all the tracked pools
// Reserves will be modified on every block due to Sync events
pub struct PoolManager {
    // All addresses that hte pool is tracking
    addresses: FxHashSet<Address>,
    // Mapping from address to generic pool
    address_to_pool: FxHashMap<Address, Pool>,
    // Mapping from address to V2Pool
    address_to_v2pool: RwLock<FxHashMap<Address, UniswapV2PoolState>>,
    /// Mapping from address to V3Pool
    address_to_v3pool: RwLock<FxHashMap<Address, UniswapV3PoolState>>,
}

impl PoolManager {
    // construct a new instance
    pub async fn new(
        working_pools: Vec<Pool>,
        http: Arc<RootProvider<Http<Client>>>,
        contract_address: Address,
    ) -> Self {
        let address_to_pool = working_pools
            .iter()
            .map(|pool| (pool.address(), pool.clone()))
            .collect();
        // construct mapping and do an initial reserve sync so we are working wtih an up to date state
        let (v2_state, v3_state) = Self::initial_sync(working_pools, http, contract_address).await;
        let mut addresses: FxHashSet<Address> = v2_state.keys().cloned().collect();
        addresses.extend(v3_state.keys().cloned());
        Self {
            addresses,
            address_to_pool,
            address_to_v2pool: RwLock::new(v2_state),
            address_to_v3pool: RwLock::new(v3_state),
        }
    }

    /// Batch sync resreves for tracked pools upon startup
    /// Indirection, sync events provide address which we used to get the node indicies
    /// which are utilized by the graph
    async fn initial_sync(
        working_pools: Vec<Pool>,
        http: Arc<RootProvider<Http<Client>>>,
        contract_address: Address,
    ) -> (
        FxHashMap<Address, UniswapV2PoolState>,
        FxHashMap<Address, UniswapV3PoolState>,
    ) {
        // split into v2 and v3 pools
        let (v2_pools, v3_pools): (Vec<Pool>, Vec<Pool>) =
            working_pools.into_iter().partition(|pool| {
                pool.pool_type() == PoolType::UniswapV2
                    || pool.pool_type() == PoolType::SushiSwapV2
                    || pool.pool_type() == PoolType::PancakeSwapV2
            });
        
        let v2_pools: Vec<Address> = v2_pools.into_iter().map(|pool| pool.address()).collect();
        let v3_pools = v3_pools.into_iter().map(|pool| pool.address()).collect();

        let v2_state = Self::initial_v2_sync(v2_pools, http.clone());
        let v3_state = Self::initial_v3_sync(v3_pools, http.clone());
        let (v2_state, v3_state) = futures::join!(v2_state, v3_state);

        (v2_state, v3_state)
    }

    async fn initial_v2_sync(
        v2_pools: Vec<Address>,
        http: Arc<RootProvider<Http<Client>>>,
    ) -> FxHashMap<Address, UniswapV2PoolState> {
        info!("Start v2 sync");
        let mut v2_state: FxHashMap<Address, UniswapV2PoolState> = FxHashMap::default();
        let v2_state_snapshots = v2_pool_snapshot(v2_pools, http).await.unwrap();

        for v2_state_snapshot in v2_state_snapshots {
            v2_state.insert(v2_state_snapshot.address, v2_state_snapshot);
        }
        info!("Finished v2 sync");

        v2_state
    }

    async fn initial_v3_sync(
        v3_pools: Vec<Address>,
        http: Arc<RootProvider<Http<Client>>>,
    ) -> FxHashMap<Address, UniswapV3PoolState> {
        info!("Start v3 sync");
        let mut v3_state: FxHashMap<Address, UniswapV3PoolState> = FxHashMap::default();
        let v3_state_snapshots = v3_pool_snapshot(&v3_pools, http).await.unwrap();

        for v3_state_snapshot in v3_state_snapshots {
            v3_state.insert(v3_state_snapshot.address, v3_state_snapshot);
        }
        info!("Finished v3 sync");
        v3_state
    }

    pub fn exists(&self, address: &Address) -> bool {
        self.addresses.contains(address)
    }

    pub fn v2_update_from_snapshots(&self, v2_snapshots: Vec<UniswapV2PoolState>) {
        let mut v2_pools = self.address_to_v2pool.write().unwrap();
        for snapshot in v2_snapshots {
            v2_pools.entry(snapshot.address)
                .and_modify(|existing| {
                    existing.reserve0 = snapshot.reserve0;
                    existing.reserve1 = snapshot.reserve1;
                })
                .or_insert(snapshot);
        }
    }

    pub fn v3_update_from_snapshots(&self, v3_snapshots: Vec<UniswapV3PoolState>) {
        let mut v3_pools = self.address_to_v3pool.write().unwrap();
        for snapshot in v3_snapshots {
            v3_pools.entry(snapshot.address)
                .and_modify(|existing| {
                    existing.liquidity = snapshot.liquidity;
                    existing.sqrt_price = snapshot.sqrt_price;
                    existing.tick = snapshot.tick;
                    existing.fee = snapshot.fee;
                    existing.tick_spacing = snapshot.tick_spacing;
                    existing.tick_bitmap = snapshot.tick_bitmap;
                    existing.ticks = snapshot.ticks;
                });
    //            .or_insert(snapshot.clone()); keep to fix the copy here
        }
    }

    pub fn get_v2pool(&self, address: &Address) -> UniswapV2PoolState {
        self.address_to_v2pool.read().unwrap().get(address).unwrap().clone()
    }

    pub fn get_v3pool(&self, address: &Address) -> UniswapV3PoolState {
        self.address_to_v3pool.read().unwrap().get(address).unwrap().clone()
    }

    pub fn zero_to_one(&self, token_in: Address, pool: &Address) -> bool {
        let pool = self.address_to_pool.get(pool).unwrap();
        token_in == pool.token0_address()
    }

}
