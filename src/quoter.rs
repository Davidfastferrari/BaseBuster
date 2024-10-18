use alloy::network::EthereumWallet;
use alloy::node_bindings::Anvil;
use alloy::primitives::{address, Address, U256};
use alloy::providers::{Provider, ProviderBuilder, WalletProvider};
use alloy::signers::local::PrivateKeySigner;
use std::sync::Arc;

use alloy::sol_types::SolCall;
use revm::db::CacheDB;
use revm::primitives::AccountInfo;
use revm::primitives::Bytecode;
use revm::primitives::TransactTo;
use std::sync::RwLock;

use alloy::eips::{BlockHashOrNumber, BlockId};
use alloy::network::Network;
use alloy::sol;
use alloy::sol_types::SolValue;
use alloy::transports::Transport;
use revm::db::AlloyDB;
use revm::primitives::{keccak256, Bytes, ExecutionResult};
use revm::Evm;

use crate::gen::{ERC20Token, FlashQuoter};
use crate::state_db::BlockStateDB;
use crate::swap::SwapPath;
/*
pub fn get_routers() -> Vec<Address> {
    vec![
        address!("4752ba5DBc23f44D87826276BF6Fd6b1C372aD24"), // UNISWAP_V2_ROUTER
        address!("6BDED42c6DA8FBf0d2bA55B2fa120C5e0c8D7891"), // SUSHISWAP_V2_ROUTER
        address!("8cFe327CEc66d1C090Dd72bd0FF11d690C33a2Eb"), // PANCAKESWAP_V2_ROUTER
        address!("327Df1E6de05895d2ab08513aaDD9313Fe505d86"), // BASESWAP_V2_ROUTER
        address!("2626664c2603336E57B271c5C0b26F421741e481"), // UNISWAP_V3_ROUTER
        address!("678Aa4bF4E210cf2166753e054d5b7c31cc7fa86"), // PANCAKESWAP_V3_ROUTER
        address!("FB7eF66a7e61224DD6FcD0D7d9C3be5C8B049b9f"), // SUSHISWAP_V3_ROUTER
        address!("1B8eea9315bE495187D873DA7773a874545D9D48"), // BASESWAP_V3_ROUTER
        address!("BE6D8f0d05cC4be24d5167a3eF062215bE6D18a5"), // SLIPSTREAM_ROUTER
        address!("cF77a3Ba9A5CA399B7c97c74d54e5b1Beb874E43"), // AERODOME_ROUTER
        address!("e20fCBdBfFC4Dd138cE8b2E6FBb6CB49777ad64D"), // AAVE_ADDRESSES_PROVIDER
        address!("BA12222222228d8Ba445958a75a0704d566BF2C8"), // BALANCER_VAULT
    ]
}
*/

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
    let _ = weth
        .approve(*flash_quoter.address(), U256::from(1e18))
        .send()
        .await
        .unwrap();

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
    db: &RwLock<BlockStateDB<T, N, P>>,
) -> U256
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N>,
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
        })
        .build();

    let approve_calldata = ERC20Token::approveCall {
        spender: quoter,
        amount: U256::from(1e18),
    }
    .abi_encode();

    evm.tx_mut().data = approve_calldata.into();

    evm.transact_commit().unwrap();

    let balance_calldata = ERC20Token::balanceOfCall {
        account: dummy_account,
    }
    .abi_encode();
    evm.tx_mut().data = balance_calldata.into();

    let res = evm.transact().unwrap().result;
    println!("This is the rest {:#?}", res);

    // make our calldata
    let calldata = FlashQuoter::quoteArbitrageCall {
        steps: quoter_path,
        amount: amount_in,
    }
    .abi_encode();

    evm.tx_mut().data = calldata.into();
    evm.tx_mut().transact_to = TransactTo::Call(quoter);

    let ref_tx = evm.transact().unwrap();
    let result = ref_tx.result;
    println!("{:#?}", result);

    match result {
        ExecutionResult::Success { output: value, .. } => {
            if let Ok(amount) = U256::abi_decode(&value.data(), false) {
                amount
            } else {
                U256::ZERO
            }
        }
        _ => U256::ZERO,
    }
}
