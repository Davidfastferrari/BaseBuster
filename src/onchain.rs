
use alloy::primitives::{U256, address, Address};
use alloy::providers::{Provider, ProviderBuilder, WalletProvider};
use alloy::network::EthereumWallet;
use alloy::node_bindings::Anvil;
use alloy::signers::local::PrivateKeySigner;
use std::sync::Arc;

use crate::swap::SwapPath;
use crate::gen::{FlashQuoter, ERC20Token};

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