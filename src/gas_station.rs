use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use tokio::sync::broadcast::Receiver;
use alloy::eips::eip1559::BaseFeeParams;
use alloy::eips::calc_next_block_base_fee;
use crate::events::Event;

pub struct GasStation {
    base_fee: AtomicU64,
    priority_fee: AtomicU64
}

// Handles all gas state and calculations
impl GasStation {
    pub fn new() -> Self {
        Self {
            base_fee: AtomicU64::new(0),
            priority_fee: AtomicU64::new(30000000),
        }
    }

    // Get the max fee and priority fee to use for this block
    pub fn get_gas_fees(&self) -> (u128, u128) {
        let base_fee = self.base_fee.load(Ordering::Relaxed) as u128;
        let priority_fee = self.priority_fee.load(Ordering::Relaxed) as u128;
        (base_fee + priority_fee, priority_fee)
    }

    // Continuously update the gas fees
    pub async fn update_gas(&self, mut block_rx: Receiver<Event>) {
        let base_fee_params = BaseFeeParams::optimism_canyon();

        while let Ok(Event::NewBlock(block)) = block_rx.recv().await {
            let base_fee = block.header.base_fee_per_gas.unwrap();
            let gas_used = block.header.gas_used;
            let gas_limit = block.header.gas_limit;

            let next_base_fee = calc_next_block_base_fee(gas_used, gas_limit, base_fee, base_fee_params);

            self.base_fee.store(next_base_fee, Ordering::Relaxed);
        }
    }
}
