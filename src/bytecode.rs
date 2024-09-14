use std::str::FromStr;
use lazy_static::lazy_static;
use revm::primitives::Bytecode;
use revm::primitives::Bytes;
use alloy::primitives::B256;

lazy_static! {
    pub static ref UNISWAP_V2_BYTECODE: Bytecode = {
        let bytecode_hex = "";
        Bytecode::new_raw(Bytes::from_str(bytecode_hex).expect("failed to decode bytecode"))
    };

    pub static ref UNISWAP_V2_CODE_HASH: B256 = UNISWAP_V2_BYTECODE.hash_slow();
}


