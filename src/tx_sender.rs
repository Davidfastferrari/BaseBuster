use crate::events::Event;
use crate::gen::FlashSwap::FlashSwapInstance;
use alloy::hex;
use alloy::network::Ethereum;
use alloy::network::EthereumWallet;
use alloy::providers::fillers::BlobGasFiller;
use alloy::providers::fillers::{
    ChainIdFiller, FillProvider, GasFiller, JoinFill, NonceFiller, WalletFiller,
};
use alloy::providers::RootProvider;
use alloy::providers::{Identity, Provider, ProviderBuilder};
use alloy::signers::k256::SecretKey;
use alloy::signers::local::PrivateKeySigner;
use alloy::transports::http::{Client, Http};
use log::{info, warn};
use std::sync::mpsc::Receiver;
use std::sync::Arc;

use crate::gen::FlashSwap;

type WalletProvider = Arc<
    FillProvider<
        JoinFill<
            JoinFill<
                Identity,
                JoinFill<GasFiller, JoinFill<BlobGasFiller, JoinFill<NonceFiller, ChainIdFiller>>>,
            >,
            WalletFiller<EthereumWallet>,
        >,
        RootProvider<Http<Client>>,
        Http<Client>,
        Ethereum,
    >,
>;
type FlashSwapContract = FlashSwapInstance<
    Http<Client>,
    Arc<
        FillProvider<
            JoinFill<
                JoinFill<
                    Identity,
                    JoinFill<
                        GasFiller,
                        JoinFill<BlobGasFiller, JoinFill<NonceFiller, ChainIdFiller>>,
                    >,
                >,
                WalletFiller<EthereumWallet>,
            >,
            RootProvider<Http<Client>>,
            Http<Client>,
            Ethereum,
        >,
    >,
>;

pub struct TransactionSender {
    wallet_provider: WalletProvider,
    contract: FlashSwapContract,
    //recent_transactions: Mutex<HashSet<Vec<u8>>>,
}

impl TransactionSender {
    pub fn new() -> Self {
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
            //recent_transactions: Mutex::new(HashSet::new()),
        }
    }
    pub async fn send_transactions(&self, tx_receiver: Receiver<Event>) {
        // wait for a new transaction that has passed simulation
        while let Ok(Event::ArbPath((arb_path, optimized_input, block_number))) = tx_receiver.recv()
        {
            info!("Sending path...");
            // convert from seacher format into swapper format
            let converted_path: Vec<FlashSwap::SwapStep> = arb_path.clone().into();

            // construct the transaction
            let gas = self
                .wallet_provider
                .estimate_eip1559_fees(None)
                .await
                .unwrap();
            let tx = self
                .contract
                .executeArbitrage(converted_path, optimized_input)
                .max_fee_per_gas(gas.max_fee_per_gas * 2)
                .max_priority_fee_per_gas(gas.max_priority_fee_per_gas * 2)
                .chain_id(8453)
                .gas(3_000_000)
                .into_transaction_request();

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
