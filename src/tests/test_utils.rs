use alloy::network::Ethereum;
use alloy::providers::{ProviderBuilder, RootProvider};
use alloy::sol_types::SolCall;
use alloy::transports::http::{Client, Http};
use pool_sync::*;
use revm::wiring::default::TransactTo;
use std::sync::Arc;
use alloy::sol_types::SolValue;
use std::collections::HashMap;
use tokio::sync::broadcast;
use std::sync::mpsc;
use alloy_eips::BlockId;
use revm_database::{AlloyDB, CacheDB};
use alloy::primitives::Address;
use revm::database_interface::WrapDatabaseAsync;
use revm::primitives::{address, U256, keccak256};
use revm::wiring::EthereumWiring;
use revm::Evm;

use super::test_gen::ERC20;
use crate::events::Event;
use crate::filter::filter_pools;
use crate::market_state::MarketState;
use crate::stream::stream_new_blocks;

type AlloyCacheDB = CacheDB<WrapDatabaseAsync<AlloyDB<Http<Client>, Ethereum, RootProvider<Http<Client>>>>>;
type WethEvm<'a> = Evm<'a, EthereumWiring<CacheDB<WrapDatabaseAsync<AlloyDB<Http<Client>, Ethereum, RootProvider<Http<Client>>>>>, ()>>;

// Setup evm instance with and router approved
pub fn evm_with_balance_and_approval(router: Address, token: Address) -> WethEvm<'static> {
    let rpc_url = std::env::var("FULL").unwrap().parse().unwrap();
    let client = ProviderBuilder::new().on_http(rpc_url);

    let alloy = WrapDatabaseAsync::new(AlloyDB::new(client, BlockId::latest())).unwrap();
    let mut cache_db = CacheDB::new(alloy);
 
    let account = address!("18B06aaF27d44B756FCF16Ca20C1f183EB49111f");
    let balance_slot = U256::from(3);
    // give our test account some fake WETH and ETH
    let one_ether = U256::from(1_000_000_000_000_000_000u128);
    let hashed_acc_balance_slot = keccak256((account, balance_slot).abi_encode());
    cache_db
        .insert_account_storage(token, hashed_acc_balance_slot.into(), one_ether)
        .unwrap();

    let mut evm = Evm::<EthereumWiring<AlloyCacheDB, ()>>::builder()
        .with_db(cache_db)
        .with_default_ext_ctx()
        .modify_cfg_env(|env| {
            env.disable_nonce_check = true;
        })
        .modify_tx_env(|tx| {
            tx.caller = account;
            tx.value = U256::ZERO;
        })
        .build();

    // setup approval call and transact
    let approve_calldata = ERC20::approveCall {
        spender: router,
        amount: U256::from(10e18),
    }.abi_encode();
    evm.tx_mut().transact_to = TransactTo::Call(token);
    evm.tx_mut().data = approve_calldata.into();
    evm.transact_commit().unwrap();
    evm
}

pub async fn load_and_filter_pools(pool_type: PoolType) -> (Vec<Pool>, u64) {
    dotenv::dotenv().ok();

    let pool_sync = PoolSync::builder()
        .add_pools(&[
            pool_type
        ])
        .chain(pool_sync::Chain::Base)
        .rate_limit(1000)
        .build()
        .unwrap();
    let (pools, last_synced_block) = pool_sync.sync_pools().await.unwrap();
    let pools = filter_pools(pools, 500, Chain::Base).await;
    (pools, last_synced_block)
}

pub async fn construct_market(pools: Vec<Pool>, last_synced_block: u64) -> (
    Arc<MarketState<Http<Client>, Ethereum, RootProvider<Http<Client>>>>,
    mpsc::Receiver<Event>,
) {
    // Create channels for communication
    let (block_sender, block_receiver) = broadcast::channel(10);
    let (address_sender, address_receiver) = mpsc::channel();

    // Setup provider
    let http_url = std::env::var("FULL").unwrap().parse().unwrap();
    let provider = ProviderBuilder::new().on_http(http_url);

    // Start the block stream
    tokio::task::spawn(stream_new_blocks(block_sender));

    // Initialize market state with pools and channels
    let market_state = MarketState::init_state_and_start_stream(
        pools,
        block_receiver.resubscribe(),
        address_sender,
        last_synced_block,
        provider,
    )
    .await
    .unwrap();

    // Return the market state and address receiver instead of block receiver
    (market_state, address_receiver)
}

pub fn construct_pool_map(pools: Vec<Pool>) -> HashMap<Address, Pool> {
    let mut map = HashMap::new();
    for pool in pools {
        map.insert(pool.address(), pool.clone());
    }
    map
}
