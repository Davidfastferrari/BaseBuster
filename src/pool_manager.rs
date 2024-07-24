use alloy::primitives::Address;
use alloy::providers::RootProvider;
use log::debug;
use alloy::primitives::address;
use alloy::primitives::U128;
use std::sync::RwLock;
use rustc_hash::FxHashSet;
use rustc_hash::FxHashMap;
use tokio::sync::Semaphore;
use petgraph::prelude::*;
use pool_sync::Pool;
use alloy::sol;
use pool_sync::PoolInfo;
use std::sync::Arc;
use alloy::transports::http::{Http, Client};
use futures::future::join_all;


// Pair contract to get reserves
sol!(
    #[derive(Debug)]
    #[sol(rpc)]
    contract UniswapV2Pair {
        function getReserves() external view returns (uint112 reserve0, uint112 reserve1, uint32 blockTimestamp);
    }
);


// Structure to hold all the tracked pools
// Reserves will be modified on every block due to Sync events
pub struct PoolManager {
    addresses: FxHashSet<Address>,
    address_to_reserves: RwLock<FxHashMap<Address, (U128, U128)>>,
}

impl PoolManager {
    // construct a new instance
    pub async fn new(working_pools: Vec<Pool>, http: Arc<RootProvider<Http<Client>>>) -> Self {
        // construct mapping and do an initial reserve sync so we are working wtih an up to date state
        let address_to_reserves = Self::initial_reserve_sync(working_pools, http).await;
        let addresses: FxHashSet<Address> = address_to_reserves.keys().cloned().collect();
        Self { addresses, address_to_reserves: RwLock::new(address_to_reserves) }
    }

    /// Batch sync resreves for tracked pools upon startup
    /// Indirection, sync events provide address which we used to get the node indicies
    /// which are utilized by the graph 
    async fn initial_reserve_sync(
        working_pools: Vec<Pool>, 
        http: Arc<RootProvider<Http<Client>>>
    ) -> FxHashMap<Address, (U128, U128)> {
        let mut address_to_reserves = FxHashMap::default();
        let rate_limiter = Arc::new(Semaphore::new(100));

        // all our fetching futures
        let futures = working_pools.iter().map(|pool| {
            // pair contract, used to access reserves
            let contract = UniswapV2Pair::new(pool.address(), http.clone());
            let rate_limiter = rate_limiter.clone();
            async move {
                let _permit = rate_limiter.acquire().await.unwrap();
                match contract.getReserves().call().await {
                    Ok(reserves) => Ok((pool.address(), reserves)),
                    Err(e) => Err((pool.address(), e)),
                }
            }
        });

        // await and process results
        let results = join_all(futures).await;
        for result in results {
            match result {
                Ok((addr, UniswapV2Pair::getReservesReturn { reserve0, reserve1, .. })) => {
                    debug!("Updated reserves for pool {}, reserves: {:?}, {:?}", addr, reserve0, reserve1);
                    address_to_reserves.insert(addr, (U128::from(reserve0), U128::from(reserve1)));
                },
                Err((addr, e)) => {
                    eprintln!("Failed to fetch reserves for pool {}: {}", addr, e);
                    // Optionally, implement retry logic here
                }
            }
        }
        address_to_reserves
    }


    // Got a new sync event, update the reserves
    pub fn update_reserves(&self, address: Address, reserve1: u128, reserve2: u128) {
        self.address_to_reserves.write().unwrap().insert(address, (U128::from(reserve1), U128::from(reserve2)));
    }

    // Get the reserves for a given address
    pub fn get_reserves(&self, address: &Address) -> (U128, U128) {
         self.address_to_reserves.read().unwrap().get(address).unwrap().clone()
    }

    pub fn exists(&self, address: &Address) -> bool {
        self.addresses.contains(address)
    }

}

