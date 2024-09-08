

use revm::db::CacheDB;
//use crate::db::RethDB;
use alloy::sol_types::SolCall;
use pool_sync::PoolType;
use alloy::primitives::{Address, address, U256};
use crate::FlashSwap;
use alloy::providers::{ProviderBuilder, Provider };
use revm::primitives::Bytecode;
use revm::primitives::AccountInfo;
    use revm::primitives::TransactTo;
use revm::Evm;
use alloy::sol;
//use gweiyser::protocols::uniswap::v2::UniswapV2Pool;
//use gweiyser::protocols::uniswap::v3::UniswapV3Pool;
//use gweiyser::{Chain, Gweiyser};
use alloy::network::Ethereum;
use alloy::network::EthereumWallet;
use alloy::node_bindings::Anvil;

use std::sync::Arc;
use alloy::{signers::local::PrivateKeySigner, sol_types::SolValue};
use revm::primitives::{keccak256, Bytes};
use crate::tests::test_utils::FlashQuoter;


sol!(
    #[derive(Debug)]
    contract Approval {
        function approve(address spender, uint256 amount) external returns (bool);
        function deposit(uint256 amount) external;
    }
);



#[cfg(test)]
mod test_sim {

    use super::*;


    pub async fn contract_sim() {
        /* 
        let url = std::env::var("FULL").unwrap();
        let amount = U256::from(1e16);
        let provider = ProviderBuilder::new().on_http(url.parse().unwrap());

        // deploy anvil and construct a signer provider
        let anvil = Anvil::new()
            .fork(url)
            .port(9100_u16)
            .try_spawn()
            .unwrap();
        let signer: PrivateKeySigner = anvil.keys()[0].clone().into();
        let wallet = EthereumWallet::from(signer);
        let anvil_signer = Arc::new(
            ProviderBuilder::new()
                .with_recommended_fillers()
                .wallet(wallet)
                .on_http(anvil.endpoint_url()));

        // setup the contract
        let flash_quoter_address = Address::ZERO;
        let flash_quoter = FlashQuoter::new(flash_quoter_address, anvil_signer.clone());

        // give the account some weth and approve contract to spend it
        let gweiyser = Gweiyser::new(anvil_signer.clone(), Chain::Base);
        let weth = gweiyser.token(address!("4200000000000000000000000000000000000006")).await;
        weth.deposit(amount).await; 
        weth.approve(*flash_quoter.address(), amount).await;

        // our path
        let path = vec![
            FlashQuoter::SwapStep {
                poolAddress: address!("88A43bbDF9D098eEC7bCEda4e2494615dfD9bB9C"),
                tokenIn: address!("4200000000000000000000000000000000000006"),
                tokenOut: address!("833589fCD6eDb6E08f4c7C32D4f71b54bdA02913"),
                protocol: 0,
                fee: 0.try_into().unwrap(),
            },
            FlashQuoter::SwapStep {
                poolAddress: address!("d0b53D9277642d899DF5C87A3966A349A798F224"),
                tokenIn: address!("833589fCD6eDb6E08f4c7C32D4f71b54bdA02913"),
                tokenOut: address!("4200000000000000000000000000000000000006"),
                protocol: 4,
                fee: 500.try_into().unwrap(),
            },
        ];


        let flash_quoter = FlashQuoter::new(*flash_quoter.address(), provider.clone());
        let FlashQuoter::executeArbitrageReturn { _0: profit } = flash_quoter
            .executeArbitrage(path, amount)
            .from(anvil.addresses()[0])
            .call()
            .await
            .unwrap();
        println!("Profit: {:#?}", profit);
        */
        todo!()
    }

    #[tokio::test(flavor = "multi_thread")]
    pub async fn revm_sim() {
        /* 
        dotenv::dotenv().ok();
        // setup provider
        let url = std::env::var("FULL").unwrap().parse().unwrap();
        let provider = ProviderBuilder::new().on_http(url);

        //let flash_addr = std::env::var("FLASH_ADDR").unwrap().parse().unwrap();
        let quoter_addr = std::env::var("QUOTER").unwrap().parse().unwrap();
        let weth = address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2");

        // setup the db
        let db_path = std::env::var("DB_PATH").unwrap();
        let mut db = CacheDB::new(RethDB::new(db_path, None).unwrap());

        // insert the quoter into the db
        let bytecode = Bytecode::new_raw(provider.get_code_at(quoter_addr).await.unwrap());
        let code_hash = bytecode.hash_slow();
        let acc = AccountInfo {
            balance: U256::ZERO,
            nonce: 0_u64,
            code: Some(bytecode),
            code_hash
        };
        

        // give our account ether
        let sender = address!("18B06aaF27d44B756FCF16Ca20C1f183EB49111f");
        let weth_balance_slot = U256::from(3);
        let one_ether = U256::from(5_000_000_000_000_000_000u128);
        let hashed_acc_balance_slot = keccak256((sender, weth_balance_slot).abi_encode());
        db
            .insert_account_storage(weth, hashed_acc_balance_slot.into(), one_ether)
            .unwrap();
        let acc_info = AccountInfo {
            nonce: 0_u64,
            balance: one_ether,
            code_hash: keccak256(Bytes::new()),
            code: None,
        };
        db.insert_account_info(sender, acc_info);


        // build our evm
        let mut evm = Evm::builder()
            .with_db(db)
            .modify_tx_env(|tx|{
                tx.value = U256::ZERO;
            }).build();

        let approve_calldata = Approval::approveCall {
            spender: quoter,
            amount: U256::from(1e18)
        }.abi_encode();

        evm.tx_mut().caller = account;
        evm.tx_mut().data = approve_calldata.into();
        evm.tx_mut().transact_to = TransactTo::Call(weth);

        let ref_tx = evm.transact_commit().unwrap();
        println!("Approval result: {:?}", ref_tx);
        

        let path = vec![
            FlashQuoter::SwapStep {
                poolAddress: address!("88A43bbDF9D098eEC7bCEda4e2494615dfD9bB9C"),
                tokenIn: address!("4200000000000000000000000000000000000006"),
                tokenOut: address!("833589fCD6eDb6E08f4c7C32D4f71b54bdA02913"),
                protocol: 0,
                fee: 0.try_into().unwrap(),
            },
            FlashQuoter::SwapStep {
                poolAddress: address!("d0b53D9277642d899DF5C87A3966A349A798F224"),
                tokenIn: address!("833589fCD6eDb6E08f4c7C32D4f71b54bdA02913"),
                tokenOut: address!("4200000000000000000000000000000000000006"),
                protocol: 4,
                fee: 500.try_into().unwrap(),
            },
        ];

        let calldata = FlashQuoter::executeArbitrageCall {
            steps: path,
            amount: U256::from(1e16)
        }.abi_encode();

        evm.tx_mut().data = calldata.into();
        evm.tx_mut().transact_to = TransactTo::Call(quoter);

        let ref_tx = evm.transact().unwrap();
        let result = ref_tx.result;
        println!("Result: {:#?}", result);
        */
        todo!()
    }

}