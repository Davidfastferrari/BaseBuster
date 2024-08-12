use alloy::primitives::U256;
use alloy::providers::{Provider, RootProvider};
use alloy::transports::http::{Http, Client};
use log::{debug, info, warn};
use std::sync::Arc;
use tokio::sync::broadcast::{Receiver, Sender};
use alloy::network::Ethereum;

use crate::events::Event;

const BASE_FEE_MULTIPLIER: f64 = 1.1; // 10% increase
const MAX_PRIORITY_FEE: u64 = 700_000_000; // 1.5 Gwei, adjust as needed for Base
const MIN_PRIORITY_FEE: u64 = 100_000; // 0.1 Gwei, adjust as needed for Base

pub struct GasPriceManager {
    provider: Arc<RootProvider<Http<Client>>>,
    
}
impl GasPriceManager {
    pub fn new(provider: Arc<RootProvider<Http<Client>>>) -> Self {
        Self { provider }
    }

    pub async fn update_gas_price(
        &self,
        mut block_receiver: Receiver<Event>,
        gas_sender: Sender<(U256, U256)>,
    ) {
        while let Ok(Event::NewBlock(block)) = block_receiver.recv().await {
            let new_base_fee = block.header.base_fee_per_gas.unwrap_or_default();
            //let adjusted_base_fee = (base_fee as f64 * BASE_FEE_MULTIPLIER) as u64;
            //let new_base_fee = U256::from(adjusted_base_fee);
            let new_priority_fee = self.estimate_priority_fee().await;
            let fee = self.provider.estimate_eip1559_fees(None).await.unwrap();
            let gas_price = self.provider.get_gas_price().await.unwrap();
            println!("Fee: {:?}", fee);
            println!("Gas price: {:?}", gas_price);

            // max fee per gas = base fee per gas + maxpriorityfee per gas
            // get_gas_price() = block.header.base_fee_per_gas +  estimate_priority_fee()





            info!("Gas price update - Base fee: {} wei, Priority fee: {} wei", new_base_fee, new_priority_fee);
            /* 
            match gas_sender.send((new_base_fee, new_priority_fee)) {
                Ok(_) => debug!("Gas price update sent - Base fee: {} wei, Priority fee: {} wei", new_base_fee, new_priority_fee),
                Err(e) => warn!("Failed to send gas price update: {:?}", e),
            }
            */
        }
    }

    async fn estimate_priority_fee(&self) -> U256 {
        match self.provider.get_max_priority_fee_per_gas().await {
            Ok(priority_fee) => U256::from(priority_fee),
            Err(e) => {
                warn!("Failed to estimate priority fee: {:?}. Using default.", e);
                U256::from(MIN_PRIORITY_FEE)
            }
        }
    }
}