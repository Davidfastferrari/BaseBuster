
use alloy::hex;
use alloy::network::{EthereumWallet, Network};
use alloy::primitives::{Address, U256};
use alloy::providers::{Provider, ProviderBuilder};
use alloy::signers::local::PrivateKeySigner;
use alloy::transports::http::Http;
use alloy::transports::Transport;
use alloy::network::Ethereum;
use alloy::signers::k256::SecretKey;
use anyhow::{anyhow, Result};
use log::{debug, error, info, warn};
use std::sync::Arc;
use alloy::primitives::address;
use std::time::{Duration, Instant};
use tokio::sync::broadcast::Receiver;
use tokio::time::sleep;

use crate::{market, FlashSwap, AMOUNT};
use crate::market::Market;

const MAX_RETRIES: u32 = 3;
const CONFIRMATION_BLOCKS: u64 = 2;
const TRANSACTION_TIMEOUT: Duration = Duration::from_secs(10);

pub struct TransactionSender {}

pub async fn send_transactions(
    mut tx_receiver: Receiver<Vec<FlashSwap::SwapStep>>,
    market: Arc<Market>,
) -> Result<()> {

    let key = std::env::var("PRIVATE_KEY").unwrap();
    let key_hex = hex::decode(key).unwrap();
    let key= SecretKey::from_bytes((&key_hex[..]).into()).unwrap();
    let signer = PrivateKeySigner::from(key);
    let wallet = EthereumWallet::from(signer);


    let provider = ProviderBuilder::new()
        .wallet(wallet)
        .on_http(std::env::var("FULL").unwrap().parse().unwrap());
    let contract = FlashSwap::new(address!("Da7C2a18d51fa876C4DCd4382ae452B811C2A766"), provider.clone());


    // wait for new transactions to send

    while let Ok(arb_path) = tx_receiver.recv().await {
        let public = address!("1E0294b6e4D72857B5eC467f5c2E52BDA37CA5b8");
        let nonce = provider.get_transaction_count(public).await?;
        let max_fee_per_gas = market.get_max_fee();
        let max_priority_fee_per_gas = market.get_max_priority_fee();

        let tx = contract.executeArbitrage(arb_path.clone(), U256::from(AMOUNT))
            .max_fee_per_gas(max_fee_per_gas)
            .max_priority_fee_per_gas(max_priority_fee_per_gas)
            .nonce(nonce)
            .chain_id(8453)
            .gas(1_000_000)
            .into_transaction_request();
        println!("Sending transaction...");
        match provider.send_transaction(tx).await {
            Ok(res) => {
                println!("Transaction sent: {:?}", res);
            }
            Err(e) => {
                println!("Transaction failed: {:?}", e);
                sleep(Duration::from_secs(1)).await;
            }
        }
        //let res = provider.send_transaction(tx).await?.get_receipt().await?;
        //println!("{:?}", res);

    }
    Ok(())
}


