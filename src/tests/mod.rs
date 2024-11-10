pub mod test_utils;
pub mod test_quotes;
pub mod test_sim;
pub mod onchain_quote;
mod test_gen;
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
