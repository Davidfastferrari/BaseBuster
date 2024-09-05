use alloy::primitives::Address;
use pool_sync::PoolType;
use std::hash::Hash;
use serde::{Serialize, Deserialize};

// A full representation of a path that we can swap along with its hash
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct  SwapPath {
    pub steps: Vec<SwapStep>,
    pub hash: u64
}

// A step representing an individual swap
#[derive(Debug, Clone, Serialize, Deserialize, Hash, PartialEq, Eq)]
pub struct SwapStep {
    pub pool_address: Address,
    pub token_in: Address,
    pub token_out: Address,
    pub protocol: PoolType,
    pub fee: u32,
}

// Mapping of pool type to number for contract
impl SwapStep {
    pub fn as_u8(&self) -> u8 {
        match self.protocol {
            PoolType::UniswapV2 => 0,
            PoolType::SushiSwapV2 => 1,
            PoolType::PancakeSwapV2 => 2,
            PoolType::BaseSwapV2 => 3,
            PoolType::UniswapV3 => 4,
            PoolType::PancakeSwapV3 => 5,
            PoolType::SushiSwapV3 => 6,
            PoolType::BaseSwapV3 => 7,
            PoolType::Slipstream => 8,
            PoolType::Aerodrome => 9,
            PoolType::AlienBaseV2 => 10,
            PoolType::AlienBaseV3 => 11,
            PoolType::MaverickV1 => 12,
            PoolType::MaverickV2 => 13,
            PoolType::BalancerV2 => 14,
            PoolType::CurveTwoCrypto => 15,
            PoolType::CurveTriCrypto => 16,
            _ => 16
        }
    }
}
