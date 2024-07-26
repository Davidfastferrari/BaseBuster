use alloy::network::EthereumWallet;
use alloy::node_bindings::Anvil;
use alloy::primitives::{address, Address, U256};
use alloy::providers::ext::TraceApi;
use alloy::providers::{Provider, ProviderBuilder, RootProvider, WsConnect};
use alloy::providers::ext::AnvilApi;
use alloy::rpc::types::trace::parity::TraceType;
use alloy::rpc::types::Filter;
use alloy::rpc::types::TransactionRequest;
use alloy::signers::local::PrivateKeySigner;
use alloy::sol;
use alloy::transports::http::{Client, Http};
use anyhow::Result;
use futures::StreamExt;

use log::info;
use std::sync::Arc;

sol!(
    #[derive(Debug)]
    #[sol(rpc)]
    FlashSwap,
    "src/abi/FlashSwapper.json"
);

sol!(
    #[derive(Debug)]
    #[sol(rpc)]
    contract Factory {
        function getPair(address tokenA, address tokenB) external view returns (address pair);
    }

    #[derive(Debug)]
    #[sol(rpc)]
    contract Pair {
        function getReserves() external view returns (uint112 reserve0, uint112 reserve1, uint32 blockTimestampLast);
    }

    #[derive(Debug)]
    #[sol(rpc)]
    contract WETH {
        function deposit() external payable;
        function approve(address spender, uint256 amount) public returns (bool);
        function balanceOf(address account) external view returns (uint256);
        function allowance(address owner, address spender) public view returns (uint256);
    }
);

pub async fn test_sim(provider: Arc<RootProvider<Http<Client>>>) -> Result<()> {
    // spawn anvil instance
    let fork_block = provider.get_block_number().await.unwrap();
    let url = std::env::var("HTTP")?;
    let weth_addr = address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");


    let url = "https://eth.merkle.io";
    let anvil = Anvil::new()
        .fork(url)
        .fork_block_number(fork_block)
        .try_spawn()?;
    let signer: PrivateKeySigner = anvil.keys()[0].clone().into();
    let wallet = EthereumWallet::from(signer);
    let anvil_provider = Arc::new(
        ProviderBuilder::new()
            .with_recommended_fillers()
            .wallet(wallet)
            .on_http(anvil.endpoint().parse()?),
    );
    anvil_provider.anvil_set_logging(true).await?;

    // deploy the flash_swap contract
    let flash_swap = FlashSwap::deploy(anvil_provider.clone()).await?;

    // send some weth to anvil account
    let weth_token = WETH::new(weth_addr, anvil_provider.clone());
    let _ = weth_token.deposit().value(U256::from(10e18)).send().await?;

    // get the balance of this account in weth
    let account = anvil_provider.get_accounts().await?[0];
    let WETH::balanceOfReturn { _0: balance } = weth_token.balanceOf(account).call().await?;
    println!("Balance of account: {:?}", balance);

    // approve the contract to spend the weth
    let _ = weth_token
        .approve(flash_swap.address().clone(), U256::from(10e18))
        .send()
        .await?;
    let WETH::allowanceReturn { _0: allowance } = weth_token.allowance(account, flash_swap.address().clone()).call().await?;
    println!("Allowance of contract: {:?}", allowance);

    // check allowanec from contract
    let FlashSwap::check_allowanceReturn { _0: allowance } = flash_swap.check_allowance(account).call().await?;
    println!("Allowance of contract: {:?}", allowance);

    // get the allowance of the contract
    let swap_results = flash_swap.flashSwap( U256::from(1e14))
        .from(account)
        .call()
        .await;
    println!("Swap results: {:?}", swap_results);

    Ok(())
}
