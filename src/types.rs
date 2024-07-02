/// Id of a token
pub type Token = u8;


/// Id of an exchange
pub type Exchange = u8;
















































/*



































use variant_count::VariantCount;
use crate::constants::*;
use alloy::primitives::Address;

/// Unique id's for each exchange
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ExchangeId {
    UniswapV2 = 0,
    UniswapV3 = 1,
    Aerodome = 2,
    Basescan = 3,
    // add in other exchanges
}

/// Represents the tokens we are arbing over
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, VariantCount)]
pub enum Token {
    USDC = 0,
    WETH = 1,
    DAI = 2,
    BRETT = 3,
    AERO = 4,

}


impl Token {
    pub fn address(&self) -> Address {
        match self {
            Self::USDC => USDC.into(),
            Self::WETH => WETH.into(),
            Self::DAI => DAI.into(),
            Self::BRETT => BRETT.into(),
            Self::AERO => AERO.into(),
            _ => panic!("blah")

        }
    }
}




/// A trading pair/pool
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pair {
    pub token0: Token,
    pub token1: Token,
    pub fee: u16,
    pub exchange_id: ExchangeId,
}

impl Pair {
    /// Return the pair's tokens
    pub fn tokens(&self) -> (Token, Token) {
        (self.token0, self.token1)
    }
    /// Return the pair's fee (as in uniswap v3 fee tier or uniswapV2 protocol wide fee)
    pub fn fee(&self) -> u16 {
        self.fee
    }
    /// Create a new pair (a, b) as given
    pub fn new_raw(a: Token, b: Token, fee: u16, exchange_id: ExchangeId) -> Self {
        Self {
            token0: a,
            token1: b,
            fee,
            exchange_id,
        }
    }
    /// Create a new pair (orders a/b based on their address as per Uniswap v2)
    /// `fee` denotes the pair's pool fee as in uniswap v3
    pub fn new(a: Token, b: Token, fee: u16, exchange_id: ExchangeId) -> Self {
        // optimization for univ2, always organize pair by address
        if a.address() < b.address() {
            Self {
                token0: a,
                token1: b,
                fee,
                exchange_id,
            }
        } else {
            Self {
                token0: b,
                token1: a,
                fee,
                exchange_id,
            }
        }
    }
}
*/
