use crate::bytecode::{UNISWAP_V2_BYTECODE, UNISWAP_V2_CODE_HASH};
use crate::state_db::BlockStateDB;
use std::collections::HashSet;
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
        pools: Vec<Pool>, // the pools we are serching over
        block_rx: Receiver<Block>, // receiver for new blocks
        address_tx: Sender<HashSet<Address>> // sender for touched addresses in a block
    ) -> Result<Arc<Self>> {
        let mut db = BlockStateDB::new(EmptyDB::new());


        MarketState::populate_db_with_pools(pools, &mut db);
        
        let market_state = Arc::new(Self {
            db: RwLock::new(db)
        });

        // tokio::task::spanw(Updatestate)

        Ok(market_state)
    }

    // Load in all of the pools and updated state from the chain
    async fn load_pools() -> Result<Vec<Pool>> {
        let pools: Vec<Pool> = Vec::new();

        let pool_sync = PoolSync::builder() 
            .add_pools(&[PoolType::UniswapV2]).chain(Chain::Base).build()?;
        let (pools, last_synced_block) = pool_sync.sync_pools().await?;
        Ok(pools)
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

}