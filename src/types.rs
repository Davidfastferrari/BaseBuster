

use variant_count::VariantCount;

/// Represents an asset type
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, VariantCount)]
pub enum Token {
        USDC = 0,
        WETH = 1,
}