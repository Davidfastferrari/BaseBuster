use alloy::network::Network;
use alloy::primitives::{address, Address, U256};
use alloy::providers::{Provider, ProviderBuilder};
use alloy::rpc::types::trace::geth::AccountState;
use alloy::rpc::types::BlockNumberOrTag;
use alloy::sol_types::SolValue;
use alloy::transports::Transport;
use anyhow::Result;
use log::{error, info};
use pool_sync::Pool;
use revm::primitives::{keccak256, Bytes};
use revm::state::{AccountInfo, Bytecode};
use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;
use std::sync::RwLock;
use tokio::sync::mpsc::{Receiver, Sender};

use crate::events::Event;
use crate::gen::FlashQuoter;
use crate::state_db::BlockStateDB;
use crate::tracing::debug_trace_block;

// Internal representation of the current state of the blockchain
pub struct MarketState<T, N, P>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N>,
{
    pub db: RwLock<BlockStateDB<T, N, P>>,
}

impl<T, N, P> MarketState<T, N, P>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + 'static,
{
    // constuct the market state with a populated db
    pub async fn init_state_and_start_stream(
        pools: Vec<Pool>,          // the pools we are serching over
        block_rx: Receiver<Event>, // receiver for new blocks
        address_tx: Sender<Event>, // sender for touched addresses in a block
        last_synced_block: u64,    // the last block that was synced too
        provider: P,
    ) -> Result<Arc<Self>> {
        // populate our state
        let mut db = BlockStateDB::new(provider).unwrap();
        MarketState::populate_db_with_pools(pools, &mut db);
        MarketState::populate_db_with_accounts(&mut db);

        // init the market state with the db
        let market_state = Arc::new(Self {
            db: RwLock::new(db),
        });

        // start the state updater
        tokio::spawn(Self::state_updater(
            market_state.clone(),
            block_rx,
            address_tx,
            last_synced_block,
        ));

        Ok(market_state)
    }

    // task to retrieve new blockchain state and update our db
    async fn state_updater(
        self: Arc<Self>,
        mut block_rx: Receiver<Event>,
        address_tx: Sender<Event>,
        mut last_synced_block: u64,
    ) {
        // http provider
        let http_url = std::env::var("FULL").unwrap().parse().unwrap();
        let http = Arc::new(ProviderBuilder::new().on_http(http_url));

        // stream in new blocks
        while let Some(Event::NewBlock(block)) = block_rx.recv().await {
            let block_number = block.header.number;
            if block_number <= last_synced_block {
                continue;
            }

            // trace the block to get all post state changes
            // todo!() this has to make up for lost blocks
            let updates =
                debug_trace_block(http.clone(), BlockNumberOrTag::Number(block_number), true).await;

            // update the db based on teh traces
            let updated_pools = self.process_block_trace(updates);
            info!(
                "Got {} updates in block {}",
                updated_pools.len(),
                block_number
            );

            // send the updated pools
            if let Err(e) = address_tx.send(Event::PoolsTouched(updated_pools)).await {
                error!("Failed to send updated pools");
            }

            last_synced_block = block_number;
        }
    }

    // process the block trace and update all pools that were affected
    #[inline]
    fn process_block_trace(
        &self,
        updates: Vec<BTreeMap<Address, AccountState>>,
    ) -> HashSet<Address> {
        let mut updated_pools: HashSet<Address> = HashSet::new();

        // aquire write access so we can update the db
        let mut db = self.db.write().unwrap();

        // iterate over the updates
        for (address, account_state) in updates.iter().flat_map(|btree_map| btree_map.iter()) {
            if db.tracking_pool(address) {
                db.update_all_slots(*address, account_state.clone())
                    .unwrap();
                updated_pools.insert(*address);
            }
        }
        updated_pools
    }

    // Insert pool information into the database
    fn populate_db_with_pools(pools: Vec<Pool>, db: &mut BlockStateDB<T, N, P>) {
        for pool in pools {
            if let Pool::UniswapV2(v2_pool) = pool {
                db.insert_v2(v2_pool).unwrap();
            }
        }
    }

    // Insert the quoter and dummy account into the db
    fn populate_db_with_accounts(db: &mut BlockStateDB<T, N, P>) {
        // give the dummy account some weth
        let dummy_account = address!("1E0294b6e4D72857B5eC467f5c2E52BDA37CA5b8");
        let weth = std::env::var("WETH").unwrap().parse().unwrap();
        let weth_balance_slot = U256::from(3);
        let one_ether = U256::from(1_000_000_000_000_000_000u128);
        let hashed_acc_balance_slot = keccak256((dummy_account, weth_balance_slot).abi_encode());
        db.insert_account_storage(weth, hashed_acc_balance_slot.into(), one_ether)
            .unwrap();

        let acc_info = AccountInfo {
            nonce: 0_u64,
            balance: one_ether,
            code_hash: keccak256(Bytes::new()),
            code: None,
        };
        db.insert_account_info(dummy_account, acc_info);

        // Insert the quoter contract, used for simulations
        let quoter = address!("0000000000000000000000000000000000001000");
        let quoter_bytecode = FlashQuoter::DEPLOYED_BYTECODE.clone();
        let quoter_acc_info = AccountInfo {
            nonce: 0_u64,
            balance: U256::ZERO,
            code_hash: keccak256(&quoter_bytecode),
            code: Some(Bytecode::new_raw(quoter_bytecode)),
        };
        db.insert_account_info(quoter, quoter_acc_info);
    }
}
