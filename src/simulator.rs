use alloy::primitives::U256;
use alloy::primitives::address;
use std::sync::mpsc::{Receiver, Sender};
//use tokio::sync::mpsc::{Receiver, Sender};
use revm::db::CacheDB;
use alloy::sol_types::SolCall;
use revm::primitives::Bytecode;
use revm::primitives::AccountInfo;
use revm::primitives::TransactTo;
use revm::Evm;
use revm::db::AlloyDB;
use alloy::sol;
use alloy::eips::{BlockId, BlockHashOrNumber};

use crate::swap::SwapStep;
use crate::events::Event;

use alloy::sol_types::SolValue;
use revm::primitives::{keccak256, Bytes, ExecutionResult};

use crate::{AMOUNT};
sol!(
    #[derive(Debug)]
    #[sol(rpc)]
    FlashQuoter,
    "src/abi/FlashQuoter.json"
);

sol!(
    #[derive(Debug)]
    contract Approval {
        function approve(address spender, uint256 amount) external returns (bool);
        function deposit(uint256 amount) external;
    }
);
// recieve a stream of potential arbitrage paths from the searcher and
// simulate them against the contract to determine if they are actually viable
pub fn simulate_paths(
    tx_sender: Sender<Event>,
    mut arb_receiver: Receiver<Event>,
) {
    /* *
    //let mut evm = setup_evm();
    //let quoter = address!("0000000000000000000000000000000000001000");
    //let provider = ProviderBuilder::new().on_http(std::env::var("FULL").unwrap().parse().unwrap());
    let mut db = CacheDB::new(RethDB::new());
    //let mut db = CacheDB::new(AlloyDB::new(provider, BlockId::latest()).unwrap());

    let account = address!("1E0294b6e4D72857B5eC467f5c2E52BDA37CA5b8");
    let weth = address!("4200000000000000000000000000000000000006");
    let quoter = address!("0000000000000000000000000000000000001000");

    // Give ourselves WETH
    let weth_balance_slot = U256::from(3);
    let one_ether = U256::from(1_000_000_000_000_000_000u128);
    let hashed_acc_balance_slot = keccak256((account, weth_balance_slot).abi_encode());
    db.insert_account_storage(weth, hashed_acc_balance_slot.into(), one_ether)
        .unwrap();

    let acc_info = AccountInfo {
        nonce: 0_u64,
        balance: one_ether,
        code_hash: keccak256(Bytes::new()),
        code: None,
    };
    db.insert_account_info(account, acc_info);

    // Insert quoter bytecode
    let quoter_bytecode = FlashQuoter::DEPLOYED_BYTECODE.clone();
    let quoter_acc_info = AccountInfo {
        nonce: 0_u64,
        balance: U256::ZERO,
        code_hash: keccak256(&quoter_bytecode),
        code: Some(Bytecode::new_raw(quoter_bytecode)),
    };
    db.insert_account_info(quoter, quoter_acc_info);

    let mut evm = Evm::builder()
        .with_db(db)
        .modify_tx_env(|tx| {
            tx.caller = account;
            tx.transact_to = TransactTo::Call(weth);
            tx.value = U256::ZERO;
        }).build();

    let approve_calldata = Approval::approveCall {
        spender: quoter,
        amount: U256::from(1e18)
    }.abi_encode();

    evm.tx_mut().data = approve_calldata.into();

    evm.transact_commit().unwrap();


    // wait for a new arbitrage path
    while let Ok(Event::ArbPath((arb_path, expected_out, u64))) = arb_receiver.recv() {
        // convert from searcher format into quoter format
        let converted_path: Vec<FlashQuoter::SwapStep> = arb_path
            .clone()
            .iter()
            .map(|step| FlashQuoter::SwapStep {
                poolAddress: step.pool_address,
                tokenIn: step.token_in,
                tokenOut: step.token_out,
                protocol: step.as_u8(),
                fee: step.fee.try_into().unwrap(),
            })
            .collect();

        // make our calldata
        let calldata = FlashQuoter::quoteArbitrageCall {
            steps: converted_path,
            amount: U256::from(AMOUNT)
        }.abi_encode();

        evm.tx_mut().data = calldata.into();
        evm.tx_mut().transact_to = TransactTo::Call(quoter);

        let ref_tx = evm.transact().unwrap();
        let result = ref_tx.result;

        match result {
            ExecutionResult::Success {
                output: value,
                ..
            } => {
                if let Ok(amount) = U256::abi_decode(&value.data(), false) {
                    println!("Expected {}, got {}", expected_out, amount);
                    if amount >= expected_out {
                        //match tx_sender.send(Event::ArbPath((arb_path, expected_out, u64))) {
                            //Ok(_) => debug!("Successful path sent"),
                            //Err(e) => warn!("Successful path send failed: {:?}", e),
                        //}
                    }
                }
            }
            _ => {}//println!("{:#?}", result),
        }
    }
    */
    
}