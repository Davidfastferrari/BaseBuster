use revm::database_interface::{Database, DatabaseRef};
use alloy::primitives::{Address, U256};
use pool_sync::{UniswapV2Pool, PoolType};
use revm::state::AccountInfo;
use zerocopy::IntoBytes;
use lazy_static::lazy_static;
use alloy::providers::Provider;
use alloy::transports::Transport;
use alloy::network::Network;
use anyhow::Result;

use super::BlockStateDB;
use crate::bytecode::*;

lazy_static! {
    static ref U112_MASK: U256 = (U256::from(1) << 112) - U256::from(1);
}

/// uniswapv2 db read/write related methods
impl <T, N, P> BlockStateDB<T, N, P> 
where 
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N>
{
    // insert a new uniswapv2 pool into the database
    pub fn insert_v2(&mut self, pool: UniswapV2Pool) -> Result<()> {
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

        self.pools.insert(address);

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
    pub fn get_token0(&mut self, pool: Address) -> Result<Option<Address>> {
        let token0 = self.storage(pool, U256::from(6))?;
        println!("Raw token0 value: {:?}", token0);
        if token0 == U256::ZERO {
            Ok(None)
        } else {
            Ok(Some(Address::from_word(token0.into())))
        }
    }

    // get token 1
    pub fn get_token1(&mut self, pool: Address) -> Result<Option<Address>> {
        let token1 = self.storage(pool, U256::from(7))?;
        println!("Raw token1 value: {:?}", token1);
        if token1 == U256::ZERO {
            Ok(None)
        } else {
            Ok(Some(Address::from_word(token1.into())))
        }
    }

    // insert pool reserves into the database
    fn insert_reserves(&mut self, pool: Address, reserve0: U256, reserve1: U256) -> Result<()> {
        self.pools.insert(pool);
        let packed_reserves = (reserve1 << 112) | reserve0;
        self.insert_account_storage(pool, U256::from(8), packed_reserves)
    }

    // insert token0 into the database
    fn insert_token0(&mut self, pool: Address, token: Address) -> Result<()> {
        let mut bytes = [0u8; 32];
        bytes[12..].copy_from_slice(token.as_bytes());
        self.insert_account_storage(pool, U256::ZERO, U256::from_be_bytes(bytes))
    }

    // insert token1 into the database
    fn insert_token1(&mut self, pool: Address, token: Address) -> Result<()> {
        let mut bytes = [0u8; 32];
        bytes[12..].copy_from_slice(token.as_bytes());
        self.insert_account_storage(pool, U256::from(1), U256::from_be_bytes(bytes))
    }

}


#[cfg(test)]
mod test_db_v2 {
    use super::*;
    use log::LevelFilter;
    use alloy::primitives::{U128, address};
    use dotenv;
    use alloy::providers::ProviderBuilder;
    use revm::wiring::default::TransactTo;
    use alloy::providers::RootProvider;
    use alloy::network::Ethereum;
    use alloy::transports::http::{Http, Client};
    use alloy::sol_types::SolCall;
    use revm::wiring::EthereumWiring;
    use std::time::Instant;
    use revm::Evm;
    use alloy::sol;

    #[test]
    pub fn test_insert_pool_and_retrieve() {
        dotenv::dotenv().ok();
        let url = std::env::var("FULL").unwrap().parse().unwrap();
        let provider = ProviderBuilder::new().on_http(url);
        let mut db = BlockStateDB::new(provider).unwrap();

        let pool_addr = address!("B4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc");
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
        //db.insert_v2(pool).unwrap();

        // asserts
        assert_eq!(db.get_token0(pool_addr).unwrap().unwrap(), token0);
        assert_eq!(db.get_token1(pool_addr).unwrap().unwrap(), token1);
        assert_eq!(db.get_reserves(&pool_addr), (U256::from(1e18), U256::from(1e16)));
    }

    #[test]
    pub fn test_fetch_pool_data() {
        dotenv::dotenv().ok();
        env_logger::Builder::new()
            .filter_level(LevelFilter::Debug) // or Info, Warn, etc.
            .init();
        let url = std::env::var("FULL").unwrap().parse().unwrap();
        let provider = ProviderBuilder::new().on_http(url);
        let mut db = BlockStateDB::new(provider).unwrap();

        let pool_addr = address!("B4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc");
        let expected_token0 = address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48");
        let expected_token1 = address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");

        // Fetch and assert token addresses
        let fetched_token1 = db.get_token1(pool_addr);
        let fetched_token0 = db.get_token0(pool_addr);
        assert_eq!(fetched_token0.unwrap().unwrap(), expected_token0, "Token0 address mismatch");
        assert_eq!(fetched_token1.unwrap().unwrap(), expected_token1, "Token1 address mismatch");

        // Fetch reserves
        let (reserve0, reserve1) = db.get_reserves(&pool_addr);
        assert!(reserve0 > U256::ZERO, "Reserve0 should be non-zero");
        assert!(reserve1 > U256::ZERO, "Reserve1 should be non-zero");
        
        println!("Fetched reserves: reserve0 = {}, reserve1 = {}", reserve0, reserve1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_get_amounts_out() {

        sol!(
            #[sol(rpc)]
            contract Uniswap {
                function getAmountsOut(uint amountIn, address[] memory path) external view returns (uint[] memory amounts);
            }
        );

        dotenv::dotenv().ok();
        let url = std::env::var("FULL").unwrap().parse().unwrap();
        let provider = ProviderBuilder::new().on_http(url);
        let mut db = BlockStateDB::new(provider.clone()).unwrap();

        let pool_addr = address!("B4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc");
        let token0 = address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"); // USDC
        let token1 = address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"); // WETH

        let amount_in = U256::from(1000000000); // 1 USDC (6 decimals)
        let calldata = Uniswap::getAmountsOutCall {
            amountIn: amount_in,
            path: vec![token0, token1],
        }.abi_encode();

        // Prepare calldata for getAmountsOut

        // Create EVM instance
        let mut evm = Evm::<EthereumWiring<&mut BlockStateDB<Http<Client>, Ethereum, RootProvider<Http<Client>>>, ()>>::builder()
            .with_db(&mut db)
            .modify_tx_env(|tx| {
                tx.caller = address!("0000000000000000000000000000000000000001");
                tx.transact_to = TransactTo::Call(address!("7a250d5630B4cF539739dF2C5dAcb4c659F2488D"));
                tx.data = calldata.into();
                tx.value = U256::ZERO;
            }).build();

        
        let start = Instant::now();
        let ref_tx = evm.transact().unwrap();
        println!("First Took {:?}", start.elapsed());

        let end = Instant::now();
        let ref_tx = evm.transact().unwrap();
        println!("Second Took {:?}", end.elapsed());
        //println!("{:?}", ref_tx);
        //let result = ref_tx.result; 

        //println!("{:?}", result);
    }
}
