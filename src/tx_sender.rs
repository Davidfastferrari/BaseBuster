use crate::events::Event;
use alloy::hex;
use alloy::network::EthereumWallet;
use alloy::providers::{Provider, ProviderBuilder};
use alloy::signers::k256::SecretKey;
use alloy::signers::local::PrivateKeySigner;
use std::time::Instant;
use log::{info, warn};
use std::sync::mpsc::Receiver;
use std::sync::Arc;

use crate::gen::FlashSwap;
use crate::types::{WalletProvider, FlashSwapContract};
use crate::gas_station::GasStation;

pub struct TransactionSender {
    wallet_provider: WalletProvider,
    contract: FlashSwapContract,
    gas_station: Arc<GasStation>
    //recent_transactions: Mutex<HashSet<Vec<u8>>>,
}

impl TransactionSender {
    pub fn new(gas_station: Arc<GasStation>) -> Self {
        // construct a wallet
        let key = std::env::var("PRIVATE_KEY").unwrap();
        let key_hex = hex::decode(key).unwrap();
        let key = SecretKey::from_bytes((&key_hex[..]).into()).unwrap();
        let signer = PrivateKeySigner::from(key);
        let wallet = EthereumWallet::from(signer);

        // construct a signing provider
        //let url = std::env::var("FULL").unwrap().parse().unwrap();
        let url = "https://mempool.merkle.io/rpc/base/pk_mbs_323cf6b720ba9734112249c7eff2b88d"
            .parse()
            .unwrap();
        let wallet_provider = Arc::new(
            ProviderBuilder::new()
                .with_recommended_fillers()
                .wallet(wallet)
                .on_http(url),
        );

        // instantiate the swap contract
        let contract_address = std::env::var("SWAP_CONTRACT").unwrap();
        let contract = FlashSwap::new(contract_address.parse().unwrap(), wallet_provider.clone());

        Self {
            wallet_provider,
            contract,
            gas_station
            //recent_transactions: Mutex::new(HashSet::new()),
        }
    }
    pub async fn send_transactions(&self, tx_receiver: Receiver<Event>) {
        // wait for a new transaction that has passed simulation
        while let Ok(Event::ArbPath((arb_path, optimized_input, block_number))) = tx_receiver.recv()
        {
            info!("Sending path...");
            let start = Instant::now();
            // convert from seacher format into swapper format
            let converted_path: Vec<FlashSwap::SwapStep> = arb_path.clone().into();

            // Construct the transaction
            let (max_fee, priority_fee) = self.gas_station.get_gas_fees();
            let tx = self
                .contract
                .executeArbitrage(converted_path, optimized_input)
                .max_fee_per_gas(max_fee)
                .max_priority_fee_per_gas(priority_fee)
                .chain_id(8453)
                // Increase gas limit to ensure it doesn't fail
                .gas(4_000_000)
                .into_transaction_request();
            info!("Took {:?} to create and sign tx, sending...", start.elapsed());

            // send the transaction
            match self.wallet_provider.send_transaction(tx).await {
                Ok(tx_result) => match tx_result.get_receipt().await {
                    Ok(receipt) => {
                        //let current_block = self.wallet_provider.get_block_number().await.unwrap();
                        info!("landed {:#?}", receipt);
                    }
                    Err(e) => {
                        warn!("Failed to get transaction receipt: {:?}", e);
                    }
                },
                Err(e) => warn!("Transaction failed: {:?}", e),
            }
        }
    }
}



#[cfg(test)]
mod tx_signing_tests {
    use pool_sync::PoolType;
    use alloy::primitives::{address, U256};
    use std::time::Instant;
    use crate::swap::{SwapPath, SwapStep};

    use super::*;

    // Create a mock swappath
    fn dummy_swap_path() -> SwapPath {
        // Create a dummy swap path
        let dummy_path = vec![
            SwapStep {
                pool_address: address!("4C36388bE6F416A29C8d8Eee81C771cE6bE14B18"),
                token_in: address!("d9aAEc86B65D86f6A7B5B1b0c42FFA531710b6CA"),  
                token_out: address!("4200000000000000000000000000000000000006"),  
                protocol: PoolType::UniswapV2,
                fee: 0,
            },
            SwapStep {
                pool_address: address!("9A834b70C07C81a9FCB695573D9008d0eF23A998"),    
                token_in: address!("4200000000000000000000000000000000000006"),  
                token_out: address!("d9aAEc86B65D86f6A7B5B1b0c42FFA531710b6CA"),  
                protocol: PoolType::UniswapV2,
                fee: 0,
            },
        ];
        SwapPath {
            steps: dummy_path,
            hash: 0
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_sign() {
        // init and get all dummy state
        dotenv::dotenv().ok();
        let key = std::env::var("PRIVATE_KEY").unwrap();
        let key_hex = hex::decode(key).unwrap();
        let key = SecretKey::from_bytes((&key_hex[..]).into()).unwrap();
        let signer = PrivateKeySigner::from(key);
        let wallet = EthereumWallet::from(signer);
        let url = "https://mempool.merkle.io/rpc/base/pk_mbs_323cf6b720ba9734112249c7eff2b88d"
            .parse()
            .unwrap();
        let wallet_provider = Arc::new(
            ProviderBuilder::new()
                .with_recommended_fillers()
                .wallet(wallet)
                .on_http(url),
        );
        let contract_address = std::env::var("SWAP_CONTRACT").unwrap();
        let contract = FlashSwap::new(contract_address.parse().unwrap(), wallet_provider.clone());
        let swap_path = dummy_swap_path();

        let total_time = Instant::now();

        // benchmark conversion time 
        let convertion_time = Instant::now();
        let converted_path:  Vec<FlashSwap::SwapStep> = swap_path.clone().into();
        println!("Path convertion took {:?}", convertion_time.elapsed());

        // benchmark gas est time
        let gas_time = Instant::now();
        let gas = wallet_provider
            .estimate_eip1559_fees(None)
            .await
            .unwrap();
        println!("Gas estimation took {:?}", gas_time.elapsed());

        // benchmark tx construction
        let tx_time = Instant::now();
        let max_fee = gas.max_fee_per_gas * 5;  // 3x the suggested max fee
        let priority_fee = gas.max_priority_fee_per_gas * 30;  // 20x the suggested priority fee

        let _ = contract
            .executeArbitrage(converted_path, U256::from(10))
            .max_fee_per_gas(max_fee)
            .max_priority_fee_per_gas(priority_fee)
            .chain_id(8453)
            // Increase gas limit to ensure it doesn't fail
            .gas(4_000_000)
            .into_transaction_request();
        println!("Tx construction took {:?}", tx_time.elapsed());

        println!("Total time {:?}", total_time.elapsed());
    }
}