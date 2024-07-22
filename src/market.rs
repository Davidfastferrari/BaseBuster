
use std::sync::RwLock;
use alloy::primitives::U256;
use tokio::sync::broadcast::Receiver;
use log::info;

pub struct Market {
    gas_price: RwLock<U256>,
}

impl Market {
    // Construct an empty market, populated on first update of block
    pub fn new() -> Self {
        Self {
            gas_price: RwLock::new(U256::from(0))
        }
    }

    // updaate the gas price
    pub async fn update_gas_price(&self, mut gas_receiver: Receiver<U256>) {
        while let Ok(gas_price) = gas_receiver.recv().await {
            let mut gas_lock = self.gas_price.write().unwrap();
            *gas_lock = gas_price;
            info!("Updated gas price to {}", gas_price);
        }
    }
}


