use super::BlockStateDB;
use revm::db::{Database, DatabaseRef};
use alloy::primitives::{U256, Address};

/// uniswapv2 db read/write related methods
impl <ExtDB: Database + DatabaseRef> BlockStateDB<ExtDB> {

    // check if we are tracking this pool
    pub fn tracking_pool(&self, pool: &Address) -> bool {
        self.pools.contains(pool)
    }

    // compute zero to one
    pub fn zero_to_one(&self, pool: &Address, token_in: Address) -> bool {
        if self.tracking_pool(pool) {
            return self.pool_info.get(pool).unwrap().token0 == token_in;
        }
        false
    }

    // get the reserves
    pub fn get_reserves(&self, pool: &Address) -> Option<(U256, U256)> {
        if self.tracking_pool(pool) {
            let packed_reserves = self.storage_ref(*pool, U256::from(8)).ok()?;
            let reserve0 = packed_reserves >> 112;
            let reserve1 = packed_reserves & ((U256::from(1) << 112) - U256::from(1));
            Some((reserve0, reserve1))
        } else {
            None
        }
    }

    // get token 0
    pub fn get_token0(&self, pool: Address) -> Result<Option<Address>, <Self as DatabaseRef>::Error> {
        let token0 = self.storage_ref(pool, U256::ZERO)?;
        if token0 == U256::ZERO {
            Ok(None)
        } else {
            Ok(Some(Address::from_word(token0.into())))
        }
    }

    // get token 1
    pub fn get_token1(&self, pool: Address) -> Result<Option<Address>, <Self as DatabaseRef>::Error> {
        let token1 = self.storage_ref(pool, U256::from(1))?;
        if token1 == U256::ZERO {
            Ok(None)
        } else {
            Ok(Some(Address::from_word(token1.into())))
        }
    }

    // insert pool reserves into the database
    pub fn insert_reserves(&mut self, pool: Address, reserve0: U256, reserve1: U256) {
        self.pools.insert(pool);
        let packed_reserves = (reserve0 << 112) | reserve1;
        self.insert_account_storage(pool, U256::from(8), packed_reserves);
    }

    // update pool reserves
    pub fn update_reserves(&mut self, pool: Address, reserve0: U256, reserve1: U256) {
        let packed_reserves = (reserve0 << 112) | reserve1;
        self.update_account_storage(pool, U256::from(8), packed_reserves);
    }

    // insert token0 into the database
    pub fn insert_token0(&mut self, pool: Address, token: Address) {
        self.insert_account_storage(pool, U256::ZERO, U256::from_be_bytes(token.into()));
    }

    // insert token1 into the database
    pub fn insert_token1(&mut self, pool: Address, token: Address) {
        self.insert_account_storage(pool, U256::from(1),U256::from_be_bytes(token.into()));
    }

}
