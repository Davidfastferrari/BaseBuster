use alloy::eips::eip6110::MAINNET_DEPOSIT_CONTRACT_ADDRESS;
use alloy::primitives::Address;
use alloy::providers::RootProvider;
use log::debug;
use alloy::primitives::address;
use alloy::primitives::{U128, U256};
use std::sync::RwLock;
use rustc_hash::FxHashSet;
use rustc_hash::FxHashMap;
use futures::stream::{self, StreamExt};
use tokio::sync::Semaphore;
use petgraph::prelude::*;
use pool_sync::{Pool, PoolType};
use alloy::sol;
use pool_sync::PoolInfo;
use std::sync::Arc;
use alloy::transports::http::{Http, Client};
use futures::future::join_all;




// Structure to hold all the tracked pools
// Reserves will be modified on every block due to Sync events
pub struct PoolManager {
    // All addresses that hte pool is tracking
    addresses: FxHashSet<Address>,
    // Mapping of V2 pool address to (reserve0, reserve1)
    v2_reserves: RwLock<FxHashMap<Address, (U128, U128)>>,
    // Mapping of V3 pool address to (sqrt_price_x96, tick, liquidity)
    v3_reserves: RwLock<FxHashMap<Address, (U256, i32, U128)>>,
}


impl PoolManager {
    // construct a new instance
    pub async fn new(working_pools: Vec<Pool>, http: Arc<RootProvider<Http<Client>>>, contract_address: Address) -> Self {
        // construct mapping and do an initial reserve sync so we are working wtih an up to date state
        let (v2_reserves, v3_reserves) = Self::initial_sync(working_pools, http, contract_address).await;
        let mut addresses: FxHashSet<Address> = v2_reserves.keys().cloned().collect();
        addresses.extend(v3_reserves.keys().cloned());
        Self { 
            addresses, 
            v2_reserves: RwLock::new(v2_reserves), 
            v3_reserves: RwLock::new(v3_reserves)
        }
    }

    /// Batch sync resreves for tracked pools upon startup
    /// Indirection, sync events provide address which we used to get the node indicies
    /// which are utilized by the graph 
    async fn initial_sync(
        working_pools: Vec<Pool>, 
        http: Arc<RootProvider<Http<Client>>>,
        contract_address: Address
    ) -> (FxHashMap<Address, (U128, U128)>, FxHashMap<Address, (U256, i32, U128)>) {

        // split into v2 and v3 pools
        let (v2_pools, v3_pools): (Vec<Pool>, Vec<Pool> ) = working_pools.into_iter()
            .partition(|pool| pool.pool_type() == PoolType::UniswapV2 || pool.pool_type() == PoolType::SushiSwapV2 || pool.pool_type() == PoolType::PancakeSwapV2);

        let v2_reserves_future = Self::initial_v2_sync(v2_pools, http.clone(), contract_address);
        let v3_reserves_future = Self::initial_v3_sync(v3_pools, http.clone(), contract_address);
        let (v2_reserves, v3_reserves) = futures::join!(v2_reserves_future, v3_reserves_future);

        (v2_reserves, v3_reserves)
    }

    async fn initial_v2_sync(
        working_pools: Vec<Pool>, 
        http: Arc<RootProvider<Http<Client>>>,
        contract_address: Address
    ) -> FxHashMap<Address, (U128, U128)> {

        let mut v2_reserves: FxHashMap<Address, (U128, U128)> = FxHashMap::default();
        let contract = Arc::new(crate::BatchSync::new(contract_address, http.clone()));
        let results = stream::iter(working_pools.chunks(50))
            .map(|chunk| {
                let contract = contract.clone();
                async move {
                    let addresses: Vec<Address> = chunk.iter().map(|pool| pool.address()).collect();
                    let crate::BatchSync::syncV2Return { _0: reserves } = contract.syncV2(addresses).call().await.unwrap();
                    reserves
                }
            })
            .buffer_unordered(10)
            .flat_map(|reserves| stream::iter(reserves))
            .collect::<Vec<_>>()
            .await;

        for v2_output in results {
            v2_reserves.insert(v2_output.pairAddr, (U128::from(v2_output.reserve0), U128::from(v2_output.reserve1)));
        }
        v2_reserves
    }

    async fn initial_v3_sync(
        working_pools: Vec<Pool>, 
        http: Arc<RootProvider<Http<Client>>>,
        contract_address: Address
    ) -> FxHashMap<Address, (U256, i32, U128)> {

        let mut v3_state: FxHashMap<Address, (U256, i32, U128)> = FxHashMap::default();
        let contract = Arc::new(crate::BatchSync::new(contract_address, http.clone()));
        let results = stream::iter(working_pools.chunks(50))
            .map(|chunk| {
                let contract = contract.clone();
                async move {
                    let addresses: Vec<Address> = chunk.iter().map(|pool| pool.address()).collect();
                    let crate::BatchSync::syncV3Return { _0: reserves } = contract.syncV3(addresses).call().await.unwrap();
                    reserves
                }
            })
            .buffer_unordered(10)
            .flat_map(|reserves| stream::iter(reserves))
            .collect::<Vec<_>>()
            .await;

        for v3_output in results {
            v3_state.insert(v3_output.poolAddr, (v3_output.sqrtPriceX96, v3_output.tick, U128::from(v3_output.liquidity)));
        }
        v3_state
    }

    // Got a new sync event, update the reserves
    pub fn update_v2(&self, address: Address, reserve1: u128, reserve2: u128) {
        self.v2_reserves.write().unwrap().insert(address, (U128::from(reserve1), U128::from(reserve2)));
    }

    // Got new swap, mint, burn event, update 
    pub fn update_v3(&self, address: Address, sqrt_price_x96: U256, tick: i32, liquidity: U128) {
        self.v3_reserves.write().unwrap().insert(address, (sqrt_price_x96, tick, liquidity));
    }

    // Get the reserves for a given address
    pub fn get_v2(&self, address: &Address) -> (U128, U128) {
         self.v2_reserves.read().unwrap().get(address).unwrap().clone()
    }

    // Get the sqrtPrice, tick and liquidity for a given address
    pub fn get_v3(&self, address: &Address) -> (U256, i32, U128) {
        self.v3_reserves.read().unwrap().get(address).unwrap().clone()
    }

    pub fn exists(&self, address: &Address) -> bool {
        self.addresses.contains(address)
    }
}

