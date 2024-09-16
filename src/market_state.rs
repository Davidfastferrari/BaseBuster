use std::collections::{BTreeMap, HashSet};
use alloy::providers::ProviderBuilder;
use revm::db::EmptyDB;
use revm::{DatabaseRef, Database};
use std::sync::RwLock;
use tokio::sync::mpsc::{Sender, Receiver};
use std::sync::Arc;
use log::error;
use alloy::primitives::Address;
use pool_sync::Pool;
use anyhow::Result;
use std::time::Instant;
use alloy::rpc::types::trace::geth::AccountState;
use alloy::providers::Provider;
use alloy::transports::Transport;
use alloy::network::Network;
use alloy::rpc::types::BlockNumberOrTag;

use crate::events::Event;
use crate::state_db::BlockStateDB;
use crate::tracing::debug_trace_block;

// Internal representation of the current state of the blockchain
pub struct MarketState<T, N, P> 
where 
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N>
{
    pub db: RwLock<BlockStateDB<T, N, P>>
}

impl<T, N, P> MarketState<T, N, P> 
where 
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + 'static
{

    // constuct the market state with a populated db
    pub async fn init_state_and_start_stream(
        pools: Vec<Pool>,                                           // the pools we are serching over
        block_rx: Receiver<Event>,                             // receiver for new blocks
        address_tx: Sender<Event>,      // sender for touched addresses in a block
        last_synced_block: u64,                                  // the last block that was synced too
        provider: P,
    ) -> Result<Arc<Self>> {
        // populate our state
        let mut db = BlockStateDB::new(provider);
        Self::populate_db_with_pools(pools, &mut db);
        let market_state = Arc::new(Self {
            db: RwLock::new(db)
        });

        // start the state updater
        tokio::spawn(Self::state_updater(market_state.clone(), block_rx, address_tx, last_synced_block));

        Ok(market_state)
    }


    // task to retrieve new blockchain state and update our db
    async fn state_updater(self: Arc<Self>, mut block_rx: Receiver<Event>, address_tx: Sender<Event>, mut last_synced_block: u64) {
        // http provider
        let http_url = std::env::var("FULL").unwrap().parse().unwrap();
        let http = Arc::new(ProviderBuilder::new().on_http(http_url));

        // stream in new blocks
        while let Some(Event::NewBlock(block)) = block_rx.recv().await {
            let start = Instant::now();
            let block_number = block.header.number;
            if block_number <= last_synced_block {
                continue;
            }

            // trace the block to get all post state changes
            // todo!() this has to make up for lost blocks
            let updates = debug_trace_block(http.clone(), BlockNumberOrTag::Number(block_number), true).await;

            // update the db based on teh traces
            let updated_pools = self.process_block_trace(updates);

            // send the updated pools
            if let Err(e) = address_tx.send(Event::PoolsTouched(updated_pools)).await {
               error!("Failed to send updated pools");
            }

            last_synced_block = block_number;
        }
    }


    // process the block trace and update all pools that were affected
    #[inline]
    fn process_block_trace(&self, updates: Vec<BTreeMap<Address, AccountState>> ) -> HashSet<Address> {
        let mut updated_pools: HashSet<Address> = HashSet::new();

        // aquire write access so we can update the db
        let mut db = self.db.write().unwrap();

        // iterate over the updates
        for (address, account_state) in updates.iter().flat_map(|btree_map| btree_map.iter()) {
            if db.tracking_pool(address) {
                db.update_all_slots(address.clone(), account_state.clone()).unwrap();
                updated_pools.insert(*address);
            }
        }
        updated_pools
    }

    // Insert pool information into the database
    fn populate_db_with_pools<DB: Database + DatabaseRef>(pools: Vec<Pool>, db: &mut DB) {
        for pool in pools {
            if let Pool::UniswapV2(v2_pool) = pool {
                //db.insert_v2(v2_pool).unwrap();
            }
        }
    }

}