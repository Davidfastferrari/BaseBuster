use super::BlockStateDB;
use alloy::primitives::{Address, U256};
use revm::DatabaseRef;
use anyhow::Result;

// Readers for uniswapv2 variants
impl<ExtDB: DatabaseRef> BlockStateDB<ExtDB> {

    // get the reserves from a uniswapv2 pool
    pub fn get_reserves(&self, pool: Address) -> Result<(U256, U256), ExtDB::Error> {
        let packed_reserves = self.storage_ref(pool, U256::from(8))?;

        let reserves0 = packed_reserves >> 112;
        let reserves1 = packed_reserves & (( U256::from(1) << 112) - U256::from(1));

        Ok((reserves0, reserves1))
    }
}


// Readers for uniswapv3 variants
impl<ExtDB: DatabaseRef> BlockStateDB<ExtDB> {

}