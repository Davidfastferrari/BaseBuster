use alloy::primitives::{keccak256, Address, Signed, Uint, B256, I256, U160, U256};
use alloy::sol;
use log::trace;
use pool_sync::{PoolType, UniswapV2Pool, UniswapV3Pool};
use revm::database_interface::{Database, DatabaseRef};
use revm::state::AccountInfo;

use super::BlockStateDB;
use crate::bytecode::*;
use crate::state_db::blockstate_db::BlockStateDBAccount;
use alloy::network::Network;
use alloy::providers::Provider;
use alloy::transports::Transport;
use anyhow::Result;
use lazy_static::lazy_static;
use log::info;
use std::ops::{BitAnd, Shl, Shr};
use std::time::Instant;
use zerocopy::IntoBytes;

lazy_static! {
    static ref U112_MASK: U256 = (U256::from(1) << 112) - U256::from(1);
}

lazy_static! {
    static ref BITS160MASK: U256 = U256::from(1).shl(160) - U256::from(1);
    static ref BITS128MASK: U256 = U256::from(1).shl(128) - U256::from(1);
    static ref BITS24MASK: U256 = U256::from(1).shl(24) - U256::from(1);
    static ref BITS16MASK: U256 = U256::from(1).shl(16) - U256::from(1);
    static ref BITS8MASK: U256 = U256::from(1).shl(8) - U256::from(1);
    static ref BITS1MASK: U256 = U256::from(1);
}

sol!(
    #[derive(Debug)]
    contract UniswapV3 {
        function slot0() external view returns (
            uint160 sqrtPriceX96,
            int24 tick,
uint16 observationIndex,
            uint16 observationCardinality,
            uint16 observationCardinalityNext,
            uint8 feeProtocol,
            bool unlocked
        );
    }
);

/// uniswapv3 db read/write related methods
impl<T, N, P> BlockStateDB<T, N, P>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N>,
{
    pub fn insert_v3(&mut self, pool: UniswapV3Pool) -> Result<()> {
        let address = pool.address;
        let token0 = pool.token0;
        let token1 = pool.token1;
        println!("{:#?}", pool);

        // track the pool
        self.add_pool(address, token0, token1, PoolType::UniswapV3);

        // Insert all storage values
        self.insert_slot0(
            address,
            U160::from(pool.sqrt_price),
            pool.tick,
            pool.fee as u8,
        )?;
        self.insert_liquidity(address, pool.liquidity)?;

        // Insert tick-related data
        for (tick, liquidity_net) in pool.ticks {
            self.insert_tick_liquidity_net(address, tick, liquidity_net.liquidity_net)?;
        }

        // Insert tick bitmap
        for (word_pos, bitmap) in pool.tick_bitmap {
            self.insert_tick_bitmap(address, word_pos, bitmap)?;
        }

        Ok(())
    }

    fn insert_tick_bitmap(&mut self, pool: Address, tick: i16, bitmap: U256) -> Result<()> {
        trace!(
            "V3 Database: Inserting tick bitmap for tick {} in pool {}",
            tick,
            pool
        );
        // Hash slot exactly as in read operation
        let tick_bytes = I256::try_from(tick)?.to_be_bytes::<32>();
        let mut buf = tick_bytes.to_vec();
        buf.append(&mut U256::from(6).to_be_bytes::<32>().to_vec());
        let slot = keccak256(buf.as_slice());

        let account = self.accounts.get_mut(&pool).unwrap();
        account
            .storage
            .insert(U256::from_be_bytes(slot.into()), bitmap);
        Ok(())
    }

    fn insert_position(&mut self, pool: Address, position: B256, info: U256) -> Result<()> {
        trace!("V3 Database: Inserting position info in pool {}", pool);
        // Convert position to U256 exactly as in read operation
        let position: U256 = position.into();

        // Hash slot exactly as in read operation
        let mut buf = position.to_be_bytes::<32>().to_vec();
        buf.append(&mut U256::from(7).to_be_bytes::<32>().to_vec());
        let slot = keccak256(buf.as_slice());

        let account = self.accounts.get_mut(&pool).unwrap();
        account
            .storage
            .insert(U256::from_be_bytes(slot.into()), info);
        Ok(())
    }

    fn insert_liquidity(&mut self, pool: Address, liquidity: u128) -> Result<()> {
        trace!("V3 Database: Inserting liquidity for {}", pool);
        let account = self.accounts.get_mut(&pool).unwrap();
        account.storage.insert(U256::from(4), U256::from(liquidity));
        Ok(())
    }

    fn insert_tick_liquidity_net(
        &mut self,
        pool: Address,
        tick: i32,
        liquidity_net: i128,
    ) -> Result<()> {
        trace!(
            "V3 Database: Inserting tick liquidity net for tick {} in pool {}",
            tick,
            pool
        );
        // Convert signed 128-bit to unsigned representation matching the read operation
        let unsigned_liquidity = liquidity_net as u128;

        // Hash slot exactly as in read operation
        let tick_bytes = I256::try_from(tick)?.to_be_bytes::<32>();
        let mut buf = tick_bytes.to_vec();
        buf.append(&mut U256::from(5).to_be_bytes::<32>().to_vec());
        let slot = keccak256(buf.as_slice());

        // Convert to U256 and shift left by 128 bits (inverse of the right shift in read)
        let value = U256::from(unsigned_liquidity) << 128;

        let account = self.accounts.get_mut(&pool).unwrap();
        account
            .storage
            .insert(U256::from_be_bytes(slot.into()), value);
        Ok(())
    }

    fn insert_slot0(
        &mut self,
        pool: Address,
        sqrt_price: U160,
        tick: i32,
        fee_protocol: u8,
    ) -> Result<()> {
        trace!("V3 Database: Inserting slot0 for {}", pool);
        let observation_index = 0;
        let observation_cardinality = 0;
        let observation_cardinality_next = 0;
        // Pack values exactly matching the unpacking in the read operation
        let slot0 = U256::from(sqrt_price)
            | (U256::from(tick as u32) << 160)
            | (U256::from(observation_index) << (160 + 24))
            | (U256::from(observation_cardinality) << (160 + 24 + 16))
            | (U256::from(observation_cardinality_next) << (160 + 24 + 16 + 16))
            | (U256::from(fee_protocol) << (160 + 24 + 16 + 16 + 16))
            | (U256::from(1u8) << (160 + 24 + 16 + 16 + 16 + 8)); // unlocked

        let account = self.accounts.get_mut(&pool).unwrap();
        account.storage.insert(U256::from(0), slot0);
        Ok(())
    }

    pub fn slot0(&self, address: Address) -> Result<UniswapV3::slot0Return> {
        let cell = self.storage_ref(address, U256::from(0))?;
        let tick: Uint<24, 1> = ((Shr::<U256>::shr(cell, U256::from(160))) & *BITS24MASK).to();
        let tick: Signed<24, 1> = Signed::<24, 1>::from_raw(tick);
        let tick: i32 = tick.as_i32();

        let sqrt_price_x96: U160 = cell.bitand(*BITS160MASK).to();

        Ok(UniswapV3::slot0Return {
            sqrtPriceX96: sqrt_price_x96,
            tick: tick.try_into()?,
            observationIndex: ((Shr::<U256>::shr(cell, U256::from(160 + 24))) & *BITS16MASK).to(),
            observationCardinality: ((Shr::<U256>::shr(cell, U256::from(160 + 24 + 16)))
                & *BITS16MASK)
                .to(),
            observationCardinalityNext: ((Shr::<U256>::shr(cell, U256::from(160 + 24 + 16 + 16)))
                & *BITS16MASK)
                .to(),
            feeProtocol: ((Shr::<U256>::shr(cell, U256::from(160 + 24 + 16 + 16 + 16)))
                & *BITS8MASK)
                .to(),
            unlocked: ((Shr::<U256>::shr(cell, U256::from(160 + 24 + 16 + 16 + 16 + 8)))
                & *BITS1MASK)
                .to(),
        })
    }

    fn insert_observation(&mut self, pool: Address, idx: u32, observation: U256) -> Result<()> {
        trace!(
            "V3 Database: Inserting observation {} in pool {}",
            idx,
            pool
        );
        let account = self.accounts.get_mut(&pool).unwrap();

        // Convert index to storage slot using same hashing as read method
        let mut buf = U256::from(idx).to_be_bytes::<32>().to_vec();
        buf.append(&mut U256::from(8).to_be_bytes::<32>().to_vec());
        let slot = keccak256(buf.as_slice());

        account
            .storage
            .insert(U256::from_be_bytes(slot.into()), observation);
        Ok(())
    }

    pub fn fee_growth_global0_x128(&self, address: Address) -> Result<U256> {
        let value = self.storage_ref(address, U256::from(1))?;
        Ok(value)
    }

    pub fn fee_growth_global1_x128(&self, address: Address) -> Result<U256> {
        let value = self.storage_ref(address, U256::from(2))?;
        Ok(value)
    }

    pub fn protocol_fees(&self, address: Address) -> Result<U256> {
        let value = self.storage_ref(address, U256::from(3))?;
        Ok(value)
    }

    pub fn liquidity(&self, address: Address) -> Result<u128> {
        let cell = self.storage_ref(address, U256::from(4))?;
        let cell: u128 = cell.saturating_to();
        Ok(cell)
    }

    pub fn ticks_liquidity_net(&self, address: Address, tick: i32) -> Result<i128> {
        //i24
        let cell = self.read_hashed_slot(
            &address,
            &U256::from(5),
            &U256::from_be_bytes(I256::try_from(tick)?.to_be_bytes::<32>()),
        )?;
        let unsigned_liqudity: Uint<128, 2> = cell.shr(U256::from(128)).to();
        let signed_liquidity: Signed<128, 2> = Signed::<128, 2>::from_raw(unsigned_liqudity);
        let lu128: u128 = unsigned_liqudity.to();
        let li128: i128 = lu128 as i128;

        Ok(li128)
    }
    pub fn tick_bitmap(&self, address: Address, tick: i16) -> Result<U256> {
        //i16
        let cell = self.read_hashed_slot(
            &address,
            &U256::from(6),
            &U256::from_be_bytes(I256::try_from(tick)?.to_be_bytes::<32>()),
        )?;
        Ok(cell)
    }

    pub fn position_info(&self, address: Address, position: B256) -> Result<U256> {
        //i16
        let position: U256 = position.into();
        let cell = self.read_hashed_slot(&address, &U256::from(7), &position)?;
        Ok(cell)
    }

    pub fn observations(&self, address: Address, idx: u32) -> Result<U256> {
        //i16
        let cell = self.read_hashed_slot(&address, &U256::from(7), &U256::from(idx))?;
        Ok(cell)
    }


    fn read_hashed_slot(
        &self,
        account: &Address,
        hashmap_offset: &U256,
        item: &U256,
    ) -> Result<U256> {
        let mut buf = item.to_be_bytes::<32>().to_vec();
        buf.append(&mut hashmap_offset.to_be_bytes::<32>().to_vec());
        let slot: U256 = keccak256(buf.as_slice()).into();
        Ok(self.storage_ref(*account, slot)?)
    }
}

/*
UniswapV3Pool {
    address: 0xe375e4dd3fc5bf117aa00c5241dd89ddd979a2c4,
    token0: 0x0578d8a44db98b23bf096a382e016e29a5ce0ffe,
    token1: 0x27501bdd6a4753dffc399ee20eb02b304f670f50,
    token0_name: "HIGHER",
    token1_name: "INDEX",
    token0_decimals: 18,
    token1_decimals: 18,
    liquidity: 21775078430692230315408,
    sqrt_price: 4654106501023758788420274431,
    fee: 3000,
    tick: -56695,
    tick_spacing: 60,
    tick_bitmap: {
        -58: 2305843009213693952,
        57: 50216813883093446110686315385661331328818843555712276103168,
    },
    ticks: {
        -887220: TickInfo {
            liquidity_net: 14809333843350818121657,
            initialized: true,
            liquidity_gross: 14809333843350818121657,
        },
        887220: TickInfo {
            liquidity_net: -14809333843350818121657,
            initialized: true,
            liquidity_gross: 14809333843350818121657,
        },
    },
}
*/

#[cfg(test)]
mod v3_db_test {
    use super::*;
    use alloy::primitives::{address, U128};
    use alloy::sol_types::SolCall;
    use alloy::providers::ProviderBuilder;
    use std::collections::HashMap;
    use pool_sync::TickInfo;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_insert_and_retrieve() {
        dotenv::dotenv().ok();
        let url = std::env::var("FULL").unwrap().parse().unwrap();
        let provider = ProviderBuilder::new().on_http(url);
        let mut db = BlockStateDB::new(provider).unwrap();

        let pool_addr = address!("e375e4dd3fc5bf117aa00c5241dd89ddd979a2c4");
        let token0 = address!("0578d8a44db98b23bf096a382e016e29a5ce0ffe");
        let token1 = address!("27501bdd6a4753dffc399ee20eb02b304f670f50");
        let mut tick_bitmap: HashMap<i16, U256> = HashMap::new();
        tick_bitmap.insert(-58, U256::from(2305843009213693952_u128));

        let mut ticks : HashMap<i32, TickInfo> = HashMap::new();
        ticks.insert(
        -887220, TickInfo {
            liquidity_net: 14809333843350818121657,
            initialized: true,
            liquidity_gross: 14809333843350818121657,
        });

        // construct and insert pool
        let pool = UniswapV3Pool {
            address: pool_addr,
            token0,
            token1,
            token0_name: "USDC".to_string(),
            token1_name: "WETH".to_string(),
            token0_decimals: 6,
            token1_decimals: 18,
            liquidity: 21775078430692230315408,
            sqrt_price: U256::from(4654106501023758788420274431_u128),
            fee: 3000,
            tick: -56695,
            tick_spacing: 60,
            tick_bitmap,
            ticks,
        };
        db.insert_v3(pool).unwrap();
        let zero_to_one = db.zero_to_one(&pool_addr, token0);
        println!("{:#?}", zero_to_one);
        let slot0 = db.slot0(pool_addr);
        println!("{:#?}", slot0);


    }
}





























