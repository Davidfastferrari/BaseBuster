use crate::bytecode::{UNISWAP_V2_BYTECODE, UNISWAP_V2_CODE_HASH};
use crate::state_db::BlockStateDB;
use std::collections::HashSet;
use std::collections::BTreeMap;
use alloy::rpc::types::Filter;
use alloy::providers::ext::TraceApi;
use alloy::sol_types::SolEvent;
use crate::gen::*;
use log::info;
use alloy::rpc::types::Log;
use alloy::providers::{ProviderBuilder, Provider, WsConnect};
use log::error;
use revm::db::EmptyDB;
use alloy::rpc::types::Block;
use std::sync::RwLock;
use tokio::sync::mpsc::{Sender, Receiver};
use alloy::network::Network;
use alloy::transports::Transport;
use std::sync::Arc;
use alloy::primitives::U256;
use alloy::primitives::Address;
use pool_sync::{Pool, PoolType, PoolSync, Chain, PoolInfo};
use pool_sync::UniswapV2Pool;
use anyhow::Result;
use revm::primitives::AccountInfo;
use std::time::Instant;
use crate::tracing::debug_trace_block;
use alloy::rpc::types::trace::geth::AccountState;
use alloy::rpc::types::{BlockId, TransactionRequest, BlockNumberOrTag};

// Internal representation of the current state of the blockchain
pub struct MarketState {
    pub db: RwLock<BlockStateDB<EmptyDB>>
}

impl MarketState {

    // constuct the market state with a populated db
    pub async fn init_state_and_start_stream(
        pools: Vec<Pool>,                                           // the pools we are serching over
        block_rx: Receiver<Block>,                             // receiver for new blocks
        address_tx: Sender<HashSet<Address>>,      // sender for touched addresses in a block
        last_synced_block: u64,                                  // the last block that was synced too
    ) -> Result<Arc<Self>> {
        // populate our state
        let mut db = BlockStateDB::new(EmptyDB::new());
        MarketState::populate_db_with_pools(pools, &mut db);
        
        let market_state = Arc::new(Self {
            db: RwLock::new(db)
        });

        // start the state updater
        tokio::spawn(Self::state_updater(market_state.clone(), block_rx, address_tx, last_synced_block));

        Ok(market_state)
    }


    // task to retrieve new blockchain state and update our db
    async fn state_updater(self: Arc<Self>, mut block_rx: Receiver<Block>, address_tx: Sender<HashSet<Address>>, mut last_synced_block: u64) {
        // create our providers
        let http_url = std::env::var("FULL").unwrap().parse().unwrap();
        let ws_url = std::env::var("WS").unwrap();

        let http = Arc::new(ProviderBuilder::new().on_http(http_url));
        let ws = ProviderBuilder::new().on_ws(WsConnect::new(ws_url)).await.unwrap();

        // construct our filter
        // stream in new blocks
        while let Some(block) = block_rx.recv().await {
            let block_number = block.header.number;
            if block_number <= last_synced_block {
                continue;
            }


            // trace the block to get all post state changes
            let updates = debug_trace_block(http.clone(), BlockNumberOrTag::Number(block_number), true).await;

            // update the db based on teh traces
            let updated_pools = self.process_block_trace(updates);

            // send the updated pools
            //if let Err(e) = address_tx.send(updated_pools).await {
             //  error!("Failed to send updated pools");
            //}

            last_synced_block = block_number;
        }
    }


    // process the block trace and update all pools that were affected
    fn process_block_trace(&self, updates: Vec<BTreeMap<Address, AccountState>> ) -> Vec<Address> {
        let updated_pools: Vec<Address> = Vec::new();


        // aquire write access so we can update the db
        let db = self.db.write().unwrap();

        // iterate over the updates
        for (address, account_state) in updates.iter().flat_map(|btree_map| btree_map.iter()) {
            if db.tracking_pool(address) {
                // update the pool
                // db.update_account
            }
        }

        updated_pools
    }

    // Insert pool information into the database
    fn populate_db_with_pools(pools: Vec<Pool>, db: &mut BlockStateDB<EmptyDB>) {
        for pool in pools {
            if let Pool::UniswapV2(v2_pool) = pool {
                db.insert_v2(v2_pool);
            }
        }
    }

}
