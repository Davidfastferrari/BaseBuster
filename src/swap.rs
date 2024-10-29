use crate::gen::FlashQuoter;
use crate::gen::FlashSwap;
use alloy::primitives::Address;
use alloy::primitives::Uint;
use pool_sync::PoolType;
use serde::{Deserialize, Serialize};
use std::convert::From;
use std::hash::Hash;

// A full representation of a path that we can swap along with its hash
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct SwapPath {
    pub steps: Vec<SwapStep>,
    pub hash: u64,
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

// conversions
impl From<SwapPath> for Vec<FlashQuoter::SwapStep> {
    fn from(path: SwapPath) -> Self {
        path.steps.into_iter().map(|step| step.into()).collect()
    }
}

impl From<SwapStep> for FlashQuoter::SwapStep {
    fn from(step: SwapStep) -> Self {
        FlashQuoter::SwapStep {
            poolAddress: step.pool_address,
            tokenIn: step.token_in,
            tokenOut: step.token_out,
            protocol: step.as_u8(),
            fee: Uint::from(step.fee),
        }
    }
}

impl From<SwapPath> for Vec<FlashSwap::SwapStep> {
    fn from(path: SwapPath) -> Self {
        path.steps.into_iter().map(|step| step.into()).collect()
    }
}

impl From<SwapStep> for FlashSwap::SwapStep {
    fn from(step: SwapStep) -> Self {
        FlashSwap::SwapStep {
            poolAddress: step.pool_address,
            tokenIn: step.token_in,
            tokenOut: step.token_out,
            protocol: step.as_u8(),
            fee: Uint::from(step.fee),
        }
    }
}

// Mapping of pool type to number for contract
impl SwapStep {
    pub fn as_u8(&self) -> u8 {
        match self.protocol {
            // V2 Variants
            PoolType::UniswapV2 => 0,
            PoolType::SushiSwapV2 => 1,
            PoolType::PancakeSwapV2 => 2,
            PoolType::BaseSwapV2 => 3,
            PoolType::SwapBasedV2 => 4,
            PoolType::AlienBaseV2 => 5,
            PoolType::DackieSwapV2 => 6,

            // V3 VARIANTS
            // NO DEADLINE
            PoolType::UniswapV3 => 7,
            PoolType::AlienBaseV3 => 8,
            PoolType::DackieSwapV3 => 9,
            PoolType::PancakeSwapV3 => 10,

            // DEADLINE
            PoolType::SushiSwapV3 => 11,
            PoolType::SwapBasedV3 => 12,
            PoolType::BaseSwapV3 => 13,

            // SLIPSTREAM
            PoolType::Slipstream => 14,

            // AERODROME
            PoolType::Aerodrome => 15,

            // BALANCER
            PoolType::BalancerV2 => 16,

            // TOIMPL
            PoolType::MaverickV1 => 17,
            PoolType::MaverickV2 => 18,
            PoolType::CurveTwoCrypto => 19,
            PoolType::CurveTriCrypto => 20,
            _ => 16,
        }
    }
}
