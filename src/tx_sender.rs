
use alloy::hex;
use alloy::providers::fillers::{FillProvider, JoinFill, NonceFiller, WalletFiller};
use alloy::network::EthereumWallet;
use alloy::primitives::U256;
use zerocopy::AsBytes;
use alloy::providers::{Identity, Provider, ProviderBuilder};
use alloy::signers::local::PrivateKeySigner;
use tokio::sync::Mutex;
use alloy::signers::k256::SecretKey;
use anyhow::Result;
use sha2::{Digest, Sha256};
use log::{info, warn};
use std::collections::HashSet;
use std::sync::Arc;
use alloy::primitives::address;
use std::sync::mpsc::Receiver;
use alloy::providers::RootProvider;
use alloy::transports::http::{Client, Http};
use alloy::network::Ethereum;

use crate::{FlashSwap, AMOUNT};
use crate::market::Market;

type WalletProvider = FillProvider<JoinFill<JoinFill<Identity, NonceFiller>, WalletFiller<EthereumWallet>>, RootProvider<Http<Client>>, Http<Client>, Ethereum>;

pub struct TransactionSender {
    provider: Arc<WalletProvider>,
    market: Arc<Market>,
    recent_transactions: Mutex<HashSet<Vec<u8>>>
}

impl TransactionSender {
    pub fn new(market: Arc<Market>) -> Self {
        // construct a wallet
        let key = std::env::var("PRIVATE_KEY").unwrap();
        let key_hex = hex::decode(key).unwrap();
        let key= SecretKey::from_bytes((&key_hex[..]).into()).unwrap();
        let signer = PrivateKeySigner::from(key);
        let wallet = EthereumWallet::from(signer);

        let url = "https://base-mainnet.g.alchemy.com/v2/HBCi7sGK_iS_e_oe2JqMDjHZF60O08pC".parse().unwrap();
        // construct the provider
        let provider = Arc::new(ProviderBuilder::new()
            //.with_nonce_management()
            .with_simple_nonce_management()
            .wallet(wallet)
            .on_http(url));
            //.on_http(std::env::var("FULL").unwrap().parse().unwrap()));

        Self {
            provider,
            market,
            recent_transactions: Mutex::new(HashSet::new())
        }
    }
    pub async fn send_transactions(
        &self,
        mut tx_receiver: Receiver<Vec<FlashSwap::SwapStep>>,
    ) -> Result<()> {
        let contract = FlashSwap::new(address!("94dd57290558Adb38061b6e42BF83671114b95Ac"), self.provider.clone());
        let wallet_address = address!("1E0294b6e4D72857B5eC467f5c2E52BDA37CA5b8");

        // wait for a new transaction that has passed simulation
        while let Ok(arb_path) = tx_receiver.recv() {
            println!("got the new path");
            // hash the transaction and make sure we didnt just end it
           // let tx_hash = self.hash_transaction(&arb_path);
            //let mut recent_txs = self.recent_transactions.lock().await;
            //if recent_txs.contains(&tx_hash) {
            //    info!("Already sent transaction, skipping");
            //    continue;
            //}

            // fetch information needed to send the transaction
            let max_fee_per_gas = self.market.get_max_fee();
            let max_priority_fee_per_gas = self.market.get_max_priority_fee();

            // construct and send the transaction
            info!("Sending transaction... {:#?}", arb_path);
            let tx = contract.executeArbitrage(arb_path.clone(), U256::from(AMOUNT))
                .max_fee_per_gas(max_fee_per_gas * 20 )
                .max_priority_fee_per_gas(max_priority_fee_per_gas * 20)
                .chain_id(8453)
                .gas(3_000_000)
                .into_transaction_request();

            // process the transaction receipt
            match self.provider.send_transaction(tx).await {
                Ok(tx_result) => {
                    let receipt = tx_result.get_receipt().await.unwrap();
                    info!("Transaction send: {:?}, Gas Used {}, Effective Gas Price {}", receipt.transaction_hash, receipt.gas_used, receipt.effective_gas_price);
                    //recent_txs.insert(tx_hash);
                }
                Err(e) => warn!("Transaction failed: {:?}", e),
            }
        }
        Ok(())
    }

    fn hash_transaction(&self, steps: &Vec<FlashSwap::SwapStep>) -> Vec<u8> {
        let mut hasher = Sha256::new();
        for step in steps {
            hasher.update(step.poolAddress.as_bytes());
            hasher.update(step.tokenIn.as_bytes());
            hasher.update(step.tokenOut.as_bytes());
            hasher.update(step.protocol.as_bytes());
            hasher.update(step.fee.as_le_bytes());
        };
        hasher.finalize().to_vec()
    }


}


