use revm::db::{Database, DatabaseRef};
use alloy::primitives::{Address, U256};
use pool_sync::{UniswapV2Pool, PoolType};
use revm::primitives::AccountInfo;
use zerocopy::AsBytes;
use lazy_static::lazy_static;

use super::BlockStateDB;
use crate::bytecode::*;

lazy_static! {
    static ref U112_MASK: U256 = (U256::from(1) << 112) - U256::from(1);
}

/// uniswapv2 db read/write related methods
impl <ExtDB: Database + DatabaseRef> BlockStateDB<ExtDB> {

    // insert a new uniswapv2 pool into the database
    pub fn insert_v2(&mut self, pool: UniswapV2Pool) -> Result<(), <Self as Database>::Error> {
        let address = pool.address;
        let token0 = pool.token0;
        let token1 = pool.token1;
        let reserve0 = U256::from(pool.token0_reserves);
        let reserve1 = U256::from(pool.token1_reserves);

        // add the pool account
        let account_info = AccountInfo {
            balance: U256::ZERO,
            nonce: 1,
            code_hash: *UNISWAP_V2_CODE_HASH,
            code: Some(UNISWAP_V2_BYTECODE.clone()), // insert this into contracts and set to none
        };
        self.insert_account_info(address, account_info);

        // track the pool 
        self.add_pool(address, token0, token1, PoolType::UniswapV2);

        // insert storage values
        self.insert_reserves(address, reserve0, reserve1)?;
        self.insert_token0(address, token0)?;
        self.insert_token1(address, token1)?;

        Ok(())
    }

    // check if we are tracking this pool
    #[inline]
    pub fn tracking_pool(&self, pool: &Address) -> bool {
        self.pools.contains(pool)
    }

    // compute zero to one
    pub fn zero_to_one(&self, pool: &Address, token_in: Address) -> Option<bool> {
        self.pool_info.get(pool).map(|info| info.token0 == token_in)
    }

    // get the reserves
    pub fn get_reserves(&self, pool: &Address) -> (U256, U256) {
        let value = self.storage_ref(*pool, U256::from(8)).ok().unwrap();
        ((value >> 0) & *U112_MASK, (value >> (112)) & *U112_MASK)
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
    fn insert_reserves(&mut self, pool: Address, reserve0: U256, reserve1: U256) -> Result<(), <Self as DatabaseRef>::Error> {
        self.pools.insert(pool);
        let packed_reserves = (reserve1 << 112) | reserve0;
        self.insert_account_storage(pool, U256::from(8), packed_reserves)
    }

    // insert token0 into the database
    fn insert_token0(&mut self, pool: Address, token: Address) -> Result<(), <Self as DatabaseRef>::Error> {
        let mut bytes = [0u8; 32];
        bytes[12..].copy_from_slice(token.as_bytes());
        self.insert_account_storage(pool, U256::ZERO, U256::from_be_bytes(bytes))
    }

    // insert token1 into the database
    fn insert_token1(&mut self, pool: Address, token: Address) -> Result<(), <Self as DatabaseRef>::Error> {
        let mut bytes = [0u8; 32];
        bytes[12..].copy_from_slice(token.as_bytes());
        self.insert_account_storage(pool, U256::from(1), U256::from_be_bytes(bytes))
    }

}


#[cfg(test)]
mod test_db_v2 {
    use super::*;
    use revm::db::EmptyDB;
    use alloy::primitives::{U128, address};

    #[test]
    pub fn test_insert_pool_and_retrieve() {
        let mut db = BlockStateDB::new(EmptyDB::new());

        let pool_addr = address!("1234567890123456789012345678901234567890");
        let token0 =  address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48");
        let token1 =  address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");

        // construct and insert pool
        let pool = UniswapV2Pool {
            address: pool_addr,
            token0, 
            token1, 
            token0_name: "USDC".to_string(),
            token1_name: "WETH".to_string(),
            token0_decimals: 6,
            token1_decimals: 18,
            token0_reserves: U128::from(1e18),
            token1_reserves: U128::from(1e16),
            stable: None,
            fee: None,
        };
        db.insert_v2(pool).unwrap();

        // asserts
        assert_eq!(db.get_token0(pool_addr).unwrap().unwrap(), token0);
        assert_eq!(db.get_token1(pool_addr).unwrap().unwrap(), token1);
        assert_eq!(db.get_reserves(&pool_addr), (U256::from(1e18), U256::from(1e16)));
    }
}