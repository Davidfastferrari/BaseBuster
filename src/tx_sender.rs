use crate::events::Event;
use crate::gas_station::GasStation;
use crate::gen::FlashSwap;
use alloy::eips::eip2718::Encodable2718;
use alloy::hex;
use alloy::network::{EthereumWallet, TransactionBuilder};
use alloy::primitives::Address;
use alloy::primitives::Bytes as AlloyBytes;
use alloy::providers::{Provider, ProviderBuilder};
use alloy::rpc::types::TransactionRequest;
use alloy::signers::k256::SecretKey;
use alloy::signers::local::PrivateKeySigner;
use alloy::sol_types::SolCall;
use log::info;
use reqwest::Client;
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::time::Instant;
use std::time::Duration;

pub struct TransactionSender {
    wallet: EthereumWallet,
    gas_station: Arc<GasStation>,
    contract_address: Address,
    client: Client,
    nonce: u64,
}

impl TransactionSender {
    pub async fn new(gas_station: Arc<GasStation>) -> Self {
        // construct a wallet
        let key = std::env::var("PRIVATE_KEY").unwrap();
        let key_hex = hex::decode(key).unwrap();
        let key = SecretKey::from_bytes((&key_hex[..]).into()).unwrap();
        let signer = PrivateKeySigner::from(key);
        let wallet = EthereumWallet::from(signer);

        // Create persisent http client
        let client = Client::builder()
            .pool_max_idle_per_host(10)
            .pool_idle_timeout(Duration::from_secs(90))
            .tcp_keepalive(Duration::from_secs(60))
            .tcp_nodelay(true)
            .timeout(Duration::from_secs(10))
            .connect_timeout(Duration::from_secs(5))
            .build()
            .expect("Failed to create HTTP client");

        // Warm up connection by sending a simple eth_blockNumber request
        let warmup_json = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_blockNumber",
            "params": [],
            "id": 1
        });
        let _ = client
            .post("https://mainnet-sequencer.base.org")
            .json(&warmup_json)
            .send()
            .await
            .unwrap();

        // get our starting nonce
        let provider =
            ProviderBuilder::new().on_http(std::env::var("FULL").unwrap().parse().unwrap());
        let nonce = provider
            .get_transaction_count(std::env::var("ACCOUNT").unwrap().parse().unwrap())
            .await
            .unwrap();

        Self {
            wallet,
            gas_station,
            contract_address: std::env::var("SWAP_CONTRACT").unwrap().parse().unwrap(),
            client,
            nonce,
        }
    }
    pub async fn send_transactions(&mut self, tx_receiver: Receiver<Event>) {
        // wait for a new transaction that has passed simulation
        while let Ok(Event::ArbPath((arb_path, optimized_input, block_number))) = tx_receiver.recv()
        {
            info!("Sending path...");
            let start = Instant::now();

            // construct the calldata/input
            let converted_path: Vec<FlashSwap::SwapStep> = arb_path.clone().into();
            let calldata = FlashSwap::executeArbitrageCall {
                steps: converted_path,
                amount: optimized_input,
            }
            .abi_encode();

            // Construct, sign, and encode transaction
            let (max_fee, priority_fee) = self.gas_station.get_gas_fees();
            let tx = TransactionRequest::default()
                .with_to(self.contract_address)
                .with_nonce(self.nonce)
                .with_gas_limit(2_000_000)
                .with_chain_id(8453)
                .with_max_fee_per_gas(max_fee)
                .with_max_priority_fee_per_gas(priority_fee)
                .transaction_type(2)
                .with_input(AlloyBytes::from(calldata));
            self.nonce += 1;
            let tx_envelope = tx.build(&self.wallet).await.unwrap();
            let mut encoded_tx = vec![];
            tx_envelope.encode_2718(&mut encoded_tx);
            let rlp_hex = hex::encode_prefixed(encoded_tx);

            let json = serde_json::json!({
                "jsonrpc": "2.0",
                "method": "eth_sendRawTransaction",
                "params": [rlp_hex],
                "id": 1
            });

            let res = self
                .client
                .post("https://mainnet-sequencer.base.org")
                .json(&json)
                .send()
                .await
                .unwrap();
            info!("Time to send {:?}, {}", start.elapsed(), block_number);
            let body = res.text().await.unwrap();
            println!("Response: {}", body);
        }
    }
}

#[cfg(test)]
mod tx_signing_tests {
    use crate::swap::{SwapPath, SwapStep};
    use alloy::primitives::{address, U256};
    use alloy::providers::{Provider, ProviderBuilder};
    use env_logger;
    use pool_sync::PoolType;
    use std::time::Instant;

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
            hash: 0,
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
        let converted_path: Vec<FlashSwap::SwapStep> = swap_path.clone().into();
        println!("Path convertion took {:?}", convertion_time.elapsed());

        // benchmark gas est time
        let gas_time = Instant::now();
        let gas = wallet_provider.estimate_eip1559_fees(None).await.unwrap();
        println!("Gas estimation took {:?}", gas_time.elapsed());

        // benchmark tx construction
        let tx_time = Instant::now();
        let max_fee = gas.max_fee_per_gas * 5; // 3x the suggested max fee
        let priority_fee = gas.max_priority_fee_per_gas * 30; // 20x the suggested priority fee

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

    #[tokio::test(flavor = "multi_thread")]
    async fn test_send_tx() {
        env_logger::init();
        // init environment
        dotenv::dotenv().ok();

        // Create gas station
        let gas_station = Arc::new(GasStation::new());

        // Create transaction sender
        let mut tx_sender = TransactionSender::new(gas_station).await;

        // Create a channel for sending events
        let (tx, rx) = std::sync::mpsc::channel();

        // Create and send a test event
        let swap_path = dummy_swap_path();
        let test_event = Event::ArbPath((
            swap_path,
            alloy::primitives::U256::from(1000000), // test input amount
            100u64,                                 // dummy block number
        ));

        tx.send(test_event).unwrap();

        // Send the transaction (this will only process one transaction and then exit)
        tx_sender.send_transactions(rx).await;
    }
}

// construct a signing provider
//let url = std::env::var("FULL").unwrap().parse().unwrap();
//let url = "https://mempool.merkle.io/rpc/base/pk_mbs_323cf6b720ba9734112249c7eff2b88d"
//   .parse()
//  .unwrap();
//let wallet_provider = Arc::new//(
/*
        ProviderBuilder::new()
            .with_recommended_fillers()
            .wallet(wallet)
            .on_http(url),
    );
*/

// instantiate the swap contract
//let contract_address = std::env::var("SWAP_CONTRACT").unwrap();
//let contract = FlashSwap::new(contract_address.parse().unwrap(), wallet_provider.clone());

/*
let tx = self
    .contract
    .executeArbitrage(converted_path, optimized_input)
    .max_fee_per_gas(max_fee)
    .max_priority_fee_per_gas(priority_fee)
    .chain_id(8453)
    // Increase gas limit to ensure it doesn't fail
    .gas(4_000_000)
    .into_transaction_request();
*/

/*
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
mainnet-sequencer.base.org:172.64.147.103
*/

