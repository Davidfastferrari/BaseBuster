
use alloy::primitives::{U256, address, Address};
use alloy::providers::{Provider, ProviderBuilder, WalletProvider};
use alloy::network::EthereumWallet;
use alloy::node_bindings::Anvil;
use alloy::signers::local::PrivateKeySigner;
use std::sync::Arc;

use revm::db::CacheDB;
use alloy::sol_types::SolCall;
use revm::primitives::Bytecode;
use revm::primitives::AccountInfo;
use revm::primitives::TransactTo;
use std::sync::RwLock;

use alloy::transports::Transport;
use alloy::network::Network;
use revm::Evm;
use revm::db::AlloyDB;
use alloy::sol;
use alloy::eips::{BlockId, BlockHashOrNumber};
use alloy::sol_types::SolValue;
use revm::primitives::{keccak256, Bytes, ExecutionResult};


use crate::swap::SwapPath;
use crate::gen::{FlashQuoter, ERC20Token};
use crate::state_db::BlockStateDB;

// calculates the output amount based on our custom onchain quoter contract
pub async fn onchain_out(quoter_path: Vec<FlashQuoter::SwapStep>, amount_in: U256) -> U256 {
    // deploy the quoter
    let url = std::env::var("FULL").unwrap();
    let provider = ProviderBuilder::new().on_http(url.parse().unwrap());
    let fork_block = provider.get_block_number().await.unwrap();
    let anvil = Anvil::new()
        .fork(url)
        .port(9101_u16)
        .fork_block_number(fork_block)
        .try_spawn()
        .unwrap();

    let signer: PrivateKeySigner = anvil.keys()[0].clone().into();
    let signer_pub = signer.address();
    let wallet = EthereumWallet::from(signer);
    let anvil_signer = Arc::new(
        ProviderBuilder::new()
            .with_recommended_fillers()
            .network::<alloy::network::AnyNetwork>()
            .wallet(wallet)
            .on_http(anvil.endpoint_url()),
    );

    // deploy the flash quoter
    let flash_quoter = FlashQuoter::deploy(anvil_signer.clone()).await.unwrap();

    // approve ourselves to spend weth
    let weth_addr: Address = std::env::var("WETH").unwrap().parse().unwrap();
    let weth = ERC20Token::new(weth_addr, anvil_signer.clone());
    let _ = weth.deposit().value(U256::from(1e18)).send().await.unwrap();
    let _ = weth.approve(*flash_quoter.address(), U256::from(1e18)).send().await.unwrap();
        
    // get out path into quoter form and execute the onchain quote
    match flash_quoter
        .quoteArbitrage(quoter_path, amount_in)
        .from(signer_pub)
        .call()
        .await
    {
        Ok(FlashQuoter::quoteArbitrageReturn { _0: profit }) => profit,
        Err(e) => U256::ZERO,
    }
}

pub fn revm_out<T, N, P>(
    quoter_path: Vec<FlashQuoter::SwapStep>, 
    amount_in: U256, 
    db: &RwLock<BlockStateDB<T, N, P>>
) -> U256
where 
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N>
{
    let dummy_account = address!("1E0294b6e4D72857B5eC467f5c2E52BDA37CA5b8");
    let weth = std::env::var("WETH").unwrap().parse().unwrap();
    let quoter = address!("0000000000000000000000000000000000001000");

    // Acquire the write lock outside the Evm creation
    let mut db_guard = db.write().unwrap();

    // Create the EVM with a mutable reference to the guarded database
    let mut evm = Evm::builder()
        .with_db(&mut *db_guard)
        .modify_tx_env(|tx| {
            tx.caller = dummy_account;
            tx.transact_to = TransactTo::Call(weth);
            tx.value = U256::ZERO;
        }).build();

    let approve_calldata = ERC20Token::approveCall {
        spender: quoter,
        amount: U256::from(1e18)
    }.abi_encode();

    evm.tx_mut().data = approve_calldata.into();

    evm.transact_commit().unwrap();


    let balance_calldata = ERC20Token::balanceOfCall{
        account: dummy_account
    }.abi_encode();
    evm.tx_mut().data = balance_calldata.into();

    let res = evm.transact().unwrap().result;
    println!("This is the rest {:#?}", res);

    // make our calldata
    let calldata = FlashQuoter::quoteArbitrageCall {
        steps: quoter_path,
        amount: amount_in
    }.abi_encode();

    evm.tx_mut().data = calldata.into();
    evm.tx_mut().transact_to = TransactTo::Call(quoter);



    let ref_tx = evm.transact().unwrap();
    let result = ref_tx.result;
    println!("{:#?}", result);

    match result {
        ExecutionResult::Success {
            output: value,
            ..
        } => {
            if let Ok(amount) = U256::abi_decode(&value.data(), false) {
                amount
            } else {
                U256::ZERO
            }
        }
        _ => U256::ZERO
    }
}