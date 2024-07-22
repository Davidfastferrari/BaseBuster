use alloy::primitives::Address;
use alloy::primitives::U256;

pub enum Events {
    ReserveUpdate,
}

#[derive(Debug)]
pub struct ArbPath {
    pub path: Vec<Address>,
    pub amount_in: U256
}