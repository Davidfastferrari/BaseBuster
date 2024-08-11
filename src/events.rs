use alloy::primitives::Address;
use alloy::primitives::U128;
use alloy::primitives::U256;
use alloy::rpc::types::Block;

use crate::graph::SwapStep;

#[derive(Debug, Clone)]
pub enum Event {
    // There is a new block on the chian
    NewBlock(Block),
    // We have updated the reserves for the pools based on the new block sync events
    ReserveUpdate(Vec<Address>),
    NewPath(Vec<SwapStep>),
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
