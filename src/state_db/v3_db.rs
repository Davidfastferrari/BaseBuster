use alloy::primitives::{keccak256, Address, Signed, Uint, B256, I256, U160, U256};
use alloy::sol;
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


        // make an insert new account for this pool
        let account = BlockStateDBAccount::new_not_existing();


         // makea 

        // Insert slot0
        let slot0 = U256::from(pool.sqrt_price)
            | (U256::from(pool.tick as u32) << 160)
            | (U256::from(0u16) << 184) // observationIndex
            | (U256::from(0u16) << 200) // observationCardinality
            | (U256::from(0u16) << 216) // observationCardinalityNext
            | (U256::from(0u8) << 232)  // feeProtocol
            | (U256::from(1u8) << 240); // unlocked
        //self.insert_account_storage(address, U256::from(0), slot0)?;

        todo!()
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
