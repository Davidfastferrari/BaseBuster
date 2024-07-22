
use alloy::{network::{Ethereum, EthereumWallet}, providers::{fillers::{FillProvider, JoinFill, WalletFiller}, Identity, RootProvider}, sol, transports::http::{Client, Http}};
use std::sync::Arc;
use tokio::sync::broadcast::Receiver;
use log::info;
use alloy::primitives::address;
use crate::events::ArbPath;

sol!(
    #[derive(Debug)]
    #[sol(rpc)]
    Arbor,
    "src/abi/UniswapV2FlashSwap.json"
);


// once we have found an arb, send it to the contract for execution
pub async fn send_transactions(
    provider: Arc<FillProvider<JoinFill<Identity, WalletFiller<EthereumWallet>>, RootProvider<Http<Client>>, Http<Client>, Ethereum>>,
    mut tx_receiver: Receiver<ArbPath>
) {
    while let Ok(arb_path) = tx_receiver.recv().await {
        info!("Received arb path: {:?}", arb_path);
        let path = arb_path.path;
        let amount_in = arb_path.amount_in;

        let arbor = Arbor::new(address!("8685A763F97b6228e4CF65F8B6993BFecc932e2b"), provider.clone());
        let tx = arbor.initiateFlashSwap(address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"), amount_in, path).call().await;

        match tx {
            Ok(tx) => info!("Transaction sent: {:?}", tx),
            Err(e) => info!("Transaction failed: {:?}", e)
        }
    }
}