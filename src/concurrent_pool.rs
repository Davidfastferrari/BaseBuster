use alloy::primitives::Address;
use rustc_hash::FxHashMap;
use rustc_hash::FxHashSet;
use std::sync::RwLock;
use pool_sync::Pool;

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

    // Add a new pool to be tracked
    pub fn track_pool(&mut self, address: Address, pool: Pool) {
        // add address to set
        self.pool_addrs.insert(address);

        // add address -> pool mapping
        let mut write = self.addr_to_pool.write().unwrap();
        write.insert(address, pool);
    }


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

