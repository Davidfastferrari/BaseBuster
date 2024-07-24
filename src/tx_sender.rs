use crate::events::ArbPath;
use alloy::primitives::{Address, address};
use alloy_sol_types::SolCall;
use log::info;
use std::sync::{Arc};
use tokio::sync::RwLock;
use tokio::sync::broadcast::Receiver;


use revm::{
    db::{AlloyDB, CacheDB},
    primitives::{keccak256, AccountInfo, Bytes, ExecutionResult, Output, TxKind},
    DatabaseRef, Evm,
};



use alloy::sol;
use alloy::network::{Ethereum};
use alloy::transports::http::{Http, Client};

use alloy::primitives::U256;
use alloy::eips::eip1898::BlockId;
use alloy::providers::{Provider, ProviderBuilder, RootProvider};
use alloy::rpc::types::Filter;
use alloy_sol_types::SolValue;
use revm::interpreter::Interpreter;
use revm::interpreter::{CallInputs, CreateInputs, Gas, InstructionResult};
use revm::interpreter::{CallOutcome, CreateOutcome};
use revm::precompile::primitives::Log;
use revm::Inspector;

sol!(
    #[derive(Debug)]
    #[sol(rpc)]
    Arbor,
    "src/abi/FlashSwapper.json"
);

type AlloyCacheDB = CacheDB<AlloyDB<Http<Client>, Ethereum, Arc<RootProvider<Http<Client>>>>>;


fn setup_evm() -> Evm<'static, (), CacheDB<AlloyDB<Http<Client>, Ethereum, Arc<RootProvider<Http<Client>>>>>> {
    let http_url = "http://localhost:8545";
    let http_provider = Arc::new(ProviderBuilder::new().on_http(http_url.parse().unwrap()));
    let mut cache_db = CacheDB::new(AlloyDB::new(http_provider, BlockId::default()).unwrap());

    // Setup account and WETH balance
    let account = address!("18B06aaF27d44B756FCF16Ca20C1f183EB49111f");
    let weth = address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2");
    let weth_balance_slot = U256::from(3);
    let one_ether = U256::from(1_000_000_000_000_000_000u128);
    let hashed_acc_balance_slot = keccak256((account, weth_balance_slot).abi_encode());
    cache_db
        .insert_account_storage(weth, hashed_acc_balance_slot.into(), one_ether)
        .unwrap();

    let acc_info = AccountInfo {
        nonce: 0_u64,
        balance: one_ether,
        code_hash: keccak256(Bytes::new()),
        code: None,
    };
    cache_db.insert_account_info(account, acc_info);

    Evm::builder()
        .with_db(cache_db)
        .build()
}

pub async fn send_transactions(mut tx_receiver: Receiver<ArbPath>) {
    let swapper = address!("B8d6D6b01bFe81784BE46e5771eF017Fa3c906d8");
    info!("Running send txk");

    while let Ok(arb_path) = tx_receiver.recv().await {
        info!("Received arb path: {:?}", arb_path);
        let path = arb_path.path;
        let encoded = Arbor::flashSwapCall {
            amountIn: U256::from(1e16 as u64),
            path: path,
        }
        .abi_encode();
    /* 
        let encoded = Arbor::getOutCall {
            _amountIn: U256::from(1e17 as u64),
            _path: path,
        }.abi_encode();
    */

        let mut evm = setup_evm();
        evm.tx_mut().caller = address!("0000000000000000000000000000000000000000");
        evm.tx_mut().transact_to = TxKind::Call(swapper);
        evm.tx_mut().data = encoded.into();
        evm.tx_mut().value = U256::from(0);

        let ref_tx = evm.transact().unwrap();
        let result = ref_tx.result;
        info!("{:?}", result);
        match result {
            ExecutionResult::Success {
                output: Output::Call(value),
                ..
            } => {
                let amount_out = <U256>::abi_decode(&value, false).unwrap();
                //info!("Expected out {:?}, Actual out {:?}", arb_path.expected_out, amount_out);
            }
            _ => continue,
        }

    }
}