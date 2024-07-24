use std::sync::Arc;

use alloy::{network::EthereumWallet, sol};

/* 
sol!(
    #[derive(Debug)]
    #[sol(rpc)]
    Swap,
    "src/abi/Swap.json"
);

type Signer = Arc<alloy::providers::fillers::FillProvider<alloy::providers::fillers::JoinFill<alloy::providers::Identity, alloy::providers::fillers::WalletFiller<EthereumWallet>>, alloy::providers::layers::AnvilProvider<alloy::providers::RootProvider<alloy::transports::http::Http<alloy::transports::http::Client>>, alloy::transports::http::Http<alloy::transports::http::Client>>, alloy::transports::http::Http<alloy::transports::http::Client>, alloy::network::Ethereum>>;
pub async fn deploy_swap(signer: Signer) {
    let contract = Swap::deploy(&signer).await.unwrap();
}
    */