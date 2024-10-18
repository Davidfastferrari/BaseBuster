use alloy::primitives::{address, Address, U256};
use alloy::sol_types::{SolValue, SolCall};
use revm::db::{CacheDB, EmptyDB, EmptyDBTyped};
use revm::primitives::AccountInfo;
use alloy::sol;
use revm::primitives::Bytecode;
use std::convert::Infallible;
use std::sync::mpsc::{Receiver, Sender};

use lazy_static::lazy_static;
use revm::primitives::{keccak256, Bytes, TransactTo};
use revm::Evm;

use crate::events::Event;
use crate::gen::FlashQuoter;

// Const addresses quoter
lazy_static! {
    pub static ref account: Address = address!("1E0294b6e4D72857B5eC467f5c2E52BDA37CA5b8");
    pub static ref weth: Address = std::env::var("WETH").unwrap().parse().unwrap();
    pub static ref quoter: Address = address!("0000000000000000000000000000000000001000");
}

// recieve a stream of potential arbitrage paths from the searcher and
// simulate them against the contract to determine if they are actually viable
pub fn simulate_paths(tx_sender: Sender<Event>, mut arb_receiver: Receiver<Event>) {

    // populate the db with quoter information
    let mut evm = setup_quoter();


    // wait for a new arbitrage path
    /*
    while let Ok(Event::ArbPath((arb_path, expected_out, u64))) = arb_receiver.recv() {
        // convert from searcher format into quoter format
        let converted_path: Vec<FlashQuoter::SwapStep> = arb_path.into()


        // f

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

pub fn setup_quoter() -> Evm<'static, (), CacheDB<EmptyDBTyped<Infallible>>> {

    sol!(
        #[derive(Debug)]
        contract Approval {
            function approve(address spender, uint256 amount) external returns (bool);
            function deposit(uint256 amount) external;
        }
    );
    let mut db = CacheDB::new(EmptyDB::new());

    // Give ourselves WETH
    let weth_balance_slot = U256::from(3);
    let one_ether = U256::from(1_000_000_000_000_000_000u128);
    let hashed_acc_balance_slot = keccak256((*account, weth_balance_slot).abi_encode());
    db.insert_account_storage(*weth, hashed_acc_balance_slot.into(), one_ether)
        .unwrap();
    let acc_info = AccountInfo {
        nonce: 0_u64,
        balance: one_ether,
        code_hash: keccak256(Bytes::new()),
        code: None,
    };
    db.insert_account_info(*account, acc_info);

    // Insert quoter bytecode
    let quoter_bytecode = FlashQuoter::DEPLOYED_BYTECODE.clone();
    let quoter_acc_info = AccountInfo {
        nonce: 0_u64,
        balance: U256::ZERO,
        code_hash: keccak256(&quoter_bytecode),
        code: Some(Bytecode::new_raw(quoter_bytecode)),
    };
    db.insert_account_info(*quoter, quoter_acc_info);

    let mut evm = Evm::builder()
        .with_db(db)
        .modify_tx_env(|tx| {
            tx.caller = *account;
            tx.transact_to = TransactTo::Call(*weth);
            tx.value = U256::ZERO;
        })
        .build();

    let approve_calldata = Approval::approveCall {
        spender: *quoter,
        amount: U256::from(1e18),
    }
    .abi_encode();

    evm.tx_mut().data = approve_calldata.into();
    evm.transact_commit().unwrap();
    evm
}

