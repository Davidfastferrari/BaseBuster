use alloy::primitives::Address;
use alloy::primitives::U128;
use alloy::primitives::U256;
use alloy::rpc::types::Block;

use crate::swap::SwapStep;

#[derive(Debug, Clone)]
pub enum Event {
    NewBlock(Block),
    ReserveUpdate(Vec<Address>),
    NewPath((Vec<SwapStep>, U256)),
    OptimizedPath(OptPath),
}

#[derive(Debug, Clone)]
pub struct OptPath {
    pub path: Vec<Address>,
    pub optimal_input: U256,
}

#[derive(Debug, Clone)]
pub struct ArbPath {
    pub path: Vec<Address>,
    pub reserves: Vec<(U128, U128)>,
}
