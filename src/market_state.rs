use crate::bytecode::{UNISWAP_V2_BYTECODE, UNISWAP_V2_CODE_HASH};
use crate::state_db::BlockStateDB;
use std::collections::HashSet;
use alloy::rpc::types::Filter;
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
use std::sync::Arc;
use alloy::primitives::U256;
use alloy::primitives::Address;
use pool_sync::{Pool, PoolType, PoolSync, Chain, PoolInfo};
use pool_sync::UniswapV2Pool;
use anyhow::Result;
use revm::primitives::AccountInfo;
use std::time::Instant;


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

        let http = ProviderBuilder::new().on_http(http_url);
        let ws = ProviderBuilder::new().on_ws(WsConnect::new(ws_url)).await.unwrap();

        // construct our filter
        // stream in new blocks
        while let Some(block) = block_rx.recv().await {
            let block_number = block.header.number;
            info!("Got block {}", block_number);
            if block_number <= last_synced_block {
                continue;
            }

            // fetch and process the logs
            let filter = self.create_event_filter(last_synced_block + 1, block_number);
            let logs = http.get_logs(&filter).await.unwrap();
            let updated_pools = self.process_logs(logs);
            //if let Err(e) = address_tx.send(updated_pools).await {
             //   error!("Failed to send updated pools");
            //}

            last_synced_block = block_number;
        }
    }


    // process the logs and update the db, return a set of all the pool addresses that were touched
    fn process_logs(&self, logs: Vec<Log>) -> HashSet<Address> {
        let mut updated_pools = HashSet::new();
        //let mut db = self.db.write().unwrap();

        for log in logs {
            let address = log.address();
            let topic = log.topic0().unwrap();;
            if topic == &DataEvent::Sync::SIGNATURE_HASH {
                self.process_v2_log(log);
            } 
        }
        updated_pools
    }

    // process v2 logs
    fn process_v2_log(&self, log: Log) {
        let mut db = self.db.write().unwrap();
        let sync_event = DataEvent::Sync::decode_log(log.as_ref(), true).unwrap();
        let reserves = U256::from(sync_event.reserve1 << 112) | U256::from(sync_event.reserve0 << 8);
        db.update_account_storage(sync_event.address, U256::from(8), reserves).unwrap();
        info!("updated v2");
    }
    // Insert pool information into the database
    fn populate_db_with_pools(pools: Vec<Pool>, db: &mut BlockStateDB<EmptyDB>) {
        let start = Instant::now();
        for pool in pools {
            if let Pool::UniswapV2(v2_pool) = pool {
                MarketState::insert_v2(db, v2_pool);
            }
        }
        println!("{:?}", start.elapsed());
    }


    // insert a v2 pool into the database
    fn insert_v2(db: &mut BlockStateDB<EmptyDB>, pool: UniswapV2Pool) {
        let address = pool.address;
        let token0 = pool.token0;
        let token1 = pool.token1;
        let reserve0 = U256::from(pool.token0_reserves);
        let reserve1 = U256::from(pool.token1_reserves);

        let account_info = AccountInfo {
            balance: U256::ZERO,
            nonce: 1,
            code_hash: *UNISWAP_V2_CODE_HASH,
            code: Some(UNISWAP_V2_BYTECODE.clone()),
        };

        // insert the contract
        db.insert_account_info(address, account_info);
        
        // insert the storage
        //db.insert_account_storage(address, U256::ZERO, U256::from(token0)).unwrap();
        //db.insert_account_storage(address, U256::from(1), U256::from(token1.into())).unwrap();
        let reserves = (reserve0 << 112) | (reserve1 << 8);
        db.insert_account_storage(address, U256::from(8), reserves).unwrap();
    }

    // inset a v3 pool into the database
    fn insert_v3(db: &mut BlockStateDB<EmptyDB>) {
        todo!()
    }

    // create an event filter for the logs that we want and the block range
    fn create_event_filter(&self, from_block: u64, to_block: u64) -> Filter {

        Filter::new()
            .events(&[
                //BalancerV2Event::Swap::SIGNATURE,
                //PancakeSwap::Swap::SIGNATURE,
                //AerodromeEvent::Sync::SIGNATURE,
                DataEvent::Sync::SIGNATURE,
                //DataEvent::Mint::SIGNATURE,
                //DataEvent::Burn::SIGNATURE,
                //DataEvent::Swap::SIGNATURE,
            ])
            .from_block(from_block)
            .to_block(to_block)
    }

}