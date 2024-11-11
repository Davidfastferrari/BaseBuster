#[cfg(test)]
pub use test_utils::test_utils as utils;
#[cfg(test)]
pub use onchain_quote::onchain_quote as onchain;
#[cfg(test)]
pub use offchain_quote::offchain_quote as offchain;
#[cfg(test)]
mod test_utils;
#[cfg(test)]
mod test_quotes;
#[cfg(test)]
mod onchain_quote;
#[cfg(test)]
mod test_gen;
#[cfg(test)]
mod offchain_quote;


// Tests breakdown 
// --------------------
// test_gen.rs: all abi generation needed 
// onchain_quotes.rs: logic to get a onchain quote from the quoter contract
//
//
//
//
// I need to test that all quotes are accurate, this is just qutoing functionsaltiy
// so I need to caluclate, need to quote with simulator, and need to quote onchain, 3comparisones
//
//
// need some way to test the estimator
