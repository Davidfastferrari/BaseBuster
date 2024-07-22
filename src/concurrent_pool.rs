use alloy::primitives::Address;
use alloy::providers::RootProvider;
use log::info;
use rustc_hash::FxHashMap;
use rustc_hash::FxHashSet;
use tokio::sync::Semaphore;
use std::sync::RwLock;
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
// Other than that we will have only reads
pub struct ConcurrentPool {
    addr_to_pool: RwLock<FxHashMap<Address, Pool>>,
    pool_addrs: FxHashSet<Address>
}

impl ConcurrentPool {
    // construct a new instance
    pub fn new() -> Self {
        Self {
            addr_to_pool: RwLock::new(FxHashMap::default()),
            pool_addrs: FxHashSet::default()
        }
    }

    pub async fn sync_pools(&self, http: Arc<RootProvider<Http<Client>>>) -> Result<(), Box<dyn std::error::Error>> {
        // Create a semaphore to limit concurrent requests
        let semaphore = Arc::new(Semaphore::new(100)); // Adjust this number based on your system limits

        let futures = self.pool_addrs.iter().map(|addr| {
            let http = http.clone();
            let semaphore = semaphore.clone();
            let contract = UniswapV2Pair::new(*addr, http);
            
            async move {
                let _permit = semaphore.acquire().await.unwrap();
                match contract.getReserves().call().await {
                    Ok(reserves) => Ok((*addr, reserves)),
                    Err(e) => Err((*addr, e)),
                }
            }
        });

        let results = join_all(futures).await;

        for result in results {
            match result {
                Ok((addr, UniswapV2Pair::getReservesReturn { reserve0, reserve1, .. })) => {
                    info!("Updated reserves for pool {}", addr);
                    self.update(&addr, reserve0, reserve1);
                },
                Err((addr, e)) => {
                    eprintln!("Failed to fetch reserves for pool {}: {}", addr, e);
                    // Optionally, implement retry logic here
                }
            }
        }

        Ok(())
    }

    // Add a new pool to be tracked
    pub fn track_pool(&mut self, address: Address, pool: Pool) {
        // add address to set
        self.pool_addrs.insert(address);

        // add address -> pool mapping
        let mut write = self.addr_to_pool.write().unwrap();
        write.insert(address, pool);
    }

    // get the pool for an address
    pub fn get(&self, address: &Address) -> Pool {
        self.addr_to_pool.read().unwrap()[address].clone()
    }

    // check if this exists
    pub fn exists(&self, address: &Address) -> bool {
        self.pool_addrs.contains(address)
    }

    // update the reserves of a pool
    pub fn update(&self, address: &Address, reserve1: u128, reserve2: u128) {
        let pool = self.addr_to_pool.write().unwrap();
        // need to update the pool sync code for this
    }
}

