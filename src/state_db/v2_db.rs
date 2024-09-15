use super::BlockStateDB;
use revm::db::{Database, DatabaseRef};
use alloy::primitives::{U256, Address};
use pool_sync::{UniswapV2Pool, PoolType};
use revm::primitives::AccountInfo;
use crate::bytecode::*;

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
        let reserves = (reserve0 << 112) | (reserve1 << 8);
        self.insert_account_storage(address, U256::from(8), reserves)?;
        self.insert_account_storage(address, U256::ZERO, U256::from_be_bytes(token0.into()))?;
        self.insert_account_storage(address, U256::from(1), U256::from_be_bytes(token1.into()))?;
        Ok(())
    }

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
    pub fn get_reserves(&self, pool: &Address) -> (U256, U256) {
        let packed_reserves = self.storage_ref(*pool, U256::from(8)).ok().unwrap();
        let reserve0 = packed_reserves >> 112;
        let reserve1 = packed_reserves & ((U256::from(1) << 112) - U256::from(1));
        (reserve0, reserve1)
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
    pub fn insert_reserves(&mut self, pool: Address, reserve0: U256, reserve1: U256) -> Result<(), <Self as DatabaseRef>::Error> {
        self.pools.insert(pool);
        let packed_reserves = (reserve0 << 112) | reserve1;
        self.insert_account_storage(pool, U256::from(8), packed_reserves)
    }

    // update pool reserves
    pub fn update_reserves(&mut self, pool: Address, reserve0: U256, reserve1: U256) -> Result<(), <Self as DatabaseRef>::Error>{
        let packed_reserves = (reserve0 << 112) | reserve1;
        self.update_account_storage(pool, U256::from(8), packed_reserves)
    }

    // insert token0 into the database
    pub fn insert_token0(&mut self, pool: Address, token: Address) -> Result<(), <Self as DatabaseRef>::Error>{
        self.insert_account_storage(pool, U256::ZERO, U256::from_be_bytes(token.into()))
    }

    // insert token1 into the database
    pub fn insert_token1(&mut self, pool: Address, token: Address) -> Result<(), <Self as DatabaseRef>::Error>{
        self.insert_account_storage(pool, U256::from(1),U256::from_be_bytes(token.into()))
    }

}
