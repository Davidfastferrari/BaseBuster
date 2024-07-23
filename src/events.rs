use alloy::primitives::Address;
use alloy::primitives::U256;
use alloy::rpc::types::Block;

#[derive(Debug, Clone)]
pub enum Event {
    // There is a new block on the chian
    NewBlock(Block),
    // We have updated the reserves for the pools based on the new block sync events
    ReserveUpdate,
}

#[derive(Debug, Clone)]
pub struct ArbPath {
    pub path: Vec<Address>,
    pub amount_in: U256,
    pub expected_out: U256
}