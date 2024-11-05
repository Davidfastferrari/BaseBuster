use alloy::network::Network;
use alloy::primitives::{address, Address, U256};
use alloy::providers::{Provider, ProviderBuilder, RootProvider, WsConnect};
use alloy::rpc::types::trace::geth::AccountState;
use revm::wiring::default::TransactTo;
use revm::Evm;
use std::str::FromStr;
use alloy::rpc::types::BlockNumberOrTag;
use std::time::Instant;
use alloy::transports::http::{Client, Http};
use alloy::transports::Transport;
use anyhow::Result;
use futures::StreamExt;
use alloy::sol_types::{SolCall, SolValue};
use log::{debug, error, info};
use pool_sync::Pool;
use std::collections::{BTreeMap, HashSet};
use revm::wiring::EthereumWiring;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;
use std::sync::RwLock;
use revm::primitives::keccak256;
use revm::state::{AccountInfo, Bytecode};
use pool_sync::PoolInfo;
use alloy::network::Ethereum;

use crate::swap::SwapStep;
use crate::events::Event;
use crate::gen::FlashQuoter;
use crate::state_db::{BlockStateDB, InsertionType};
use crate::tracing::debug_trace_block;

type StateDB = BlockStateDB<Http<Client>, Ethereum, RootProvider<Http<Client>>>;

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
    N: Network + Clone,
    P: Provider<T, N> + 'static + Clone,
{
    // constuct the market state with a populated db
    pub async fn init_state_and_start_stream(
        pools: Vec<Pool>,          // the pools we are serching over
        block_rx: Receiver<Event>, // receiver for new blocks
        address_tx: Sender<Event>, // sender for touched addresses in a block
        last_synced_block: u64,    // the last block that was synced too
        provider: P,
    ) -> Result<Arc<Self>> {
        debug!("Populating the db with {} pools", pools.len());

        // construct, warm up, and populate the db
        let mut db = BlockStateDB::new(provider).unwrap();
        Self::populate_db_with_pools(pools.clone(), &mut db);
        Self::warm_up_database(&pools, &mut db);

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
        block_rx: Receiver<Event>,
        address_tx: Sender<Event>,
        mut last_synced_block: u64,
    ) {
        // setup a provider for tracing
        let http_url = std::env::var("FULL").unwrap().parse().unwrap();
        let http = Arc::new(ProviderBuilder::new().on_http(http_url));

        // fast block times mean we can fall behind while initializing
        // catch up to the head to we are not missing any state
        let mut current_block = http.get_block_number().await.unwrap();
        println!("Last synced block {}", last_synced_block);

        while last_synced_block < current_block {
            debug!(
                "Catching up. Last synced block {}, Current block {}",
                last_synced_block, current_block
            );
            for block_num in (last_synced_block + 1)..=current_block {
                debug!("Processing block {block_num}");
                let _ = self.update_state(http.clone(), block_num).await;
            }
            last_synced_block = current_block;
            current_block = http.get_block_number().await.unwrap();
        }

        // start the stream
        let ws_url = WsConnect::new(std::env::var("WS").unwrap());
        let ws = Arc::new(ProviderBuilder::new().on_ws(ws_url).await.unwrap());

        let sub = ws.subscribe_blocks().await.unwrap();
        let mut stream = sub.into_stream();

        // stream in new blocks
        while let Some(block) = stream.next().await {
            let start = Instant::now();
            let block_number = block.header.number;
            if block_number <= last_synced_block {
                continue;
            }
            debug!("Processing block {block_number}");

            // update the state and get the list of updated pools
            let updated_pools = self.update_state(http.clone(), block_number).await;
            debug!("Processed the block {block_number}");

            // send the updated pools
            if let Err(e) = address_tx.send(Event::PoolsTouched(updated_pools, block_number)) {
                error!("Failed to send updated pools: {}", e);
            } else {
                info!("Block processed and send in {:?}", start.elapsed());
                debug!("Sent updated addresses for block {}", block_number);
            }

            last_synced_block = block_number;
        }
    }

    // after getting a new block, update our market state
    async fn update_state(
        &self,
        provider: Arc<RootProvider<Http<Client>>>,
        block_num: u64,
    ) -> HashSet<Address> {
        // trace the block to get all post state changes
        let updates = debug_trace_block(provider, BlockNumberOrTag::Number(block_num), true).await;

        // update the db based on teh traces
        let updated_pools = self.process_block_trace(updates);
        info!("Got {} updates in block {}", updated_pools.len(), block_num);
        updated_pools
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
                debug!("Updating state for pool {address}");
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
            if pool.is_v2() {
                db.insert_v2(pool.get_v2().unwrap().clone());
            } else if pool.is_v3() {
                db.insert_v3(pool.get_v3().unwrap().clone());
            }
        }
    }

    // this function will insert any approvals/balances we need and also
    // fetch extraneous contracts/values needed for simulation swaps and 
    // insert into the db
    fn warm_up_database(pools: &Vec<Pool>, db: &mut BlockStateDB<T, N, P>) {
        let account = address!("d8da6bf26964af9d7eed9e03e53415d37aa96045");
        let quoter: Address = address!("0000000000000000000000000000000000001000");

        // insert some default state into the db
        let ten_units  = U256::from(10_000_000_000_000_000_000u128);
        let balance_slot = keccak256((account, U256::from(3)).abi_encode());

        // insert the quoter bytecode
        let quoter_bytecode = FlashQuoter::DEPLOYED_BYTECODE.clone();
        let quoter_acc_info = AccountInfo {
            nonce: 0_u64,
            balance: U256::ZERO,
            code_hash: keccak256(&quoter_bytecode),
            code: Some(Bytecode::new_raw(quoter_bytecode)),
        };
        db.insert_account_info(quoter, quoter_acc_info, InsertionType::Custom);


        // insert the approval



        for pool in pools {
            db
                .insert_account_storage(
                    pool.token0_address(),
                    balance_slot.into(),
                    ten_units,
                    InsertionType::OnChain,
                )
                .unwrap();

            let approval_slot = U256::from_str("53551240413156709338371707309964053842040084443999429982803562122921478029054").unwrap();
            let _ = db.insert_account_storage(pool.token0_address(), approval_slot, U256::from(1e20), InsertionType::OnChain);

            // construct the evm and do basic swaps to warm it up
            let mut evm = Evm::<EthereumWiring<&mut BlockStateDB<T, N, P>, ()>>::builder()
                .with_db(db)
                .with_default_ext_ctx()
                .modify_tx_env(|tx| {
                    tx.caller = account;
                    tx.transact_to = TransactTo::Call(quoter);
                })
                .build();
            evm.cfg_mut().disable_nonce_check = true;

            // do a v2 swap
            let quote_path = vec![
                FlashQuoter::SwapStep {
                    poolAddress: pool.address(),
                    tokenIn: pool.token0_address(),
                    tokenOut: pool.token1_address(),
                    protocol: 0,
                    fee: 0.try_into().unwrap(),
                },
                FlashQuoter::SwapStep {
                    poolAddress: pool.address(),
                    tokenIn: pool.token1_address(),
                    tokenOut: pool.token0_address(),
                    protocol: 0,
                    fee: 0.try_into().unwrap(),
                },
            ];

            let quote_calldata = FlashQuoter::quoteArbitrageCall {
                steps: quote_path,
                amount: U256::from(1e15),
            }.abi_encode();
            evm.tx_mut().data = quote_calldata.into();

            // transact
            let ref_tx = evm.transact().unwrap();
            //let result = ref_tx.result;
            //let output = result.output().unwrap();
            //let decoded_outputs = <Vec<U256>>::abi_decode(output, false).unwrap();
            //println!("{:#?}", decoded_outputs);
        }


    }
}
