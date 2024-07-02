use crate::types::{Token, Exchange};

/// Edge in the grpah which is a representation
/// of a pool/ability to swap between two tokens
///
/// Contains common informaiton about the protocol 
/// this swap can occur on and the fee for the protocol
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Edge {
    pub token_in: Token,
    pub token_out: Token,
    pub exchange: Exchange,
    pub fee: u16,
}


pub trait EdgeCalc: Send + Sync  {
}


/// UniswapV2 pool represented as an edge
pub struct UniswapV2Edge {
    pub reserve_in: u64, 
    pub reserve_out: u64,
    pub fee: u16,
    pub exchange: Exchange
}

impl EdgeCalc for UniswapV2Edge {

}

/// UniswapV3 pool represented as an edge
pub struct UniswapV3Edge {
    pub sqrt_price_x96: u128,
    pub liquidity: u128,
    pub fee: u16,
    pub exchange: Exchange,
    pub zero_for_one: bool,
}


impl EdgeCalc for UniswapV3Edge {
}

