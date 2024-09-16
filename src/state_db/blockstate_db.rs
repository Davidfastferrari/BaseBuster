use alloy::primitives::{Address, U256, B256, BlockNumber};
use revm::primitives::{Account, AccountInfo, Bytecode, Log, KECCAK_EMPTY};
use alloy::primitives::address;
use revm::db::AccountState;
use revm::{Database, DatabaseCommit, DatabaseRef};
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::hash_map::Entry;
use std::future::IntoFuture;
use pool_sync::PoolType;
use alloy::rpc::types::trace::geth::AccountState as GethAccountState;
use alloy::rpc::types::BlockId;
use alloy::providers::{Provider, ProviderBuilder};
use alloy::transports::{Transport, TransportError};
use alloy::network::{BlockResponse, HeaderResponse, Network};
use tokio::runtime::Runtime;
use anyhow::Result;


#[derive(Debug, Clone)]
pub struct PoolInformation {
    pub token0: Address,
    pub token1: Address,
    pub pool_type: PoolType,
}

#[derive(Debug)]
pub struct BlockStateDB<T: Transport + Clone, N: Network, P: Provider<T, N>> {
    pub accounts: HashMap<Address, BlockStateDBAccount>,
    pub contracts: HashMap<B256, Bytecode>, 
    pub logs: Vec<Log>,
    pub block_hashes: HashMap<BlockNumber, B256>,
    pub pools: HashSet<Address>,
    pub pool_info: HashMap<Address, PoolInformation>,
    provider: P,
    runtime: Runtime,
    _marker: std::marker::PhantomData<fn() -> (T, N)>,
}

impl<T: Transport + Clone, N: Network, P: Provider<T, N>> BlockStateDB<T, N, P> {
    // Construct a new BlockStateDB
    pub fn new(provider: P) -> Self {
        let mut contracts = HashMap::new();
        contracts.insert(KECCAK_EMPTY, Bytecode::default());
        contracts.insert(B256::ZERO, Bytecode::default());

        Self {
            accounts: HashMap::new(),
            contracts,
            logs: Vec::new(),
            block_hashes: HashMap::new(),
            pools: HashSet::new(),
            pool_info: HashMap::new(),
            provider,
            runtime: Runtime::new().unwrap(),
            _marker: std::marker::PhantomData
        }
    }

    // track pool information for easy access
    pub fn add_pool(&mut self, pool: Address, token0: Address, token1: Address, pool_type: PoolType) {
        self.pools.insert(pool);
        self.pool_info.insert(pool, PoolInformation { token0, token1, pool_type });
    }

    // Insert a contract into the DB
    pub fn insert_contract(&mut self, account: &mut AccountInfo) {
        if let Some(code) = &account.code {
            if !code.is_empty() {
                if account.code_hash == KECCAK_EMPTY {
                    account.code_hash = code.hash_slow();
                }
                self.contracts
                    .entry(account.code_hash)
                    .or_insert_with(|| code.clone());
            }
        }
        if account.code_hash.is_zero() {
            account.code_hash = KECCAK_EMPTY;
        }
    }

    // insert some account info into the DB
    pub fn insert_account_info(&mut self, address: Address, mut info: AccountInfo) {
        self.insert_contract(&mut info);
        self.accounts.entry(address).or_default().info = info;
    }

    pub fn load_account(&mut self, address: Address) -> Result<&mut BlockStateDBAccount> {
        match self.accounts.entry(address) {
            Entry::Occupied(entry) => Ok(entry.into_mut()),
            Entry::Vacant(entry) => Ok(entry.insert(
                BlockStateDBAccount::new_not_existing()
            )),
        }
    }

    /// insert account storage without overriding account info
    #[inline]
    pub fn insert_account_storage(&mut self, address: Address, slot: U256, value: U256) -> Result<()> {
        let account = self.load_account(address)?;
        account.storage.insert(slot, value);
        Ok(())
    }


    // update all account storage slots for an account
    #[inline]
    pub fn update_all_slots(&mut self, address: Address, account_state: GethAccountState) ->Result<()> {
        let storage = account_state.storage;
        for (slot, value) in storage {
            self.insert_account_storage(address, slot.into(), value.into())?
        }
        Ok(())
    }
}


// Implement required Database trait
impl<T: Transport + Clone, N: Network, P: Provider<T, N>> Database for BlockStateDB<T, N, P> {
    type Error = TransportError;


    fn basic(&mut self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        // look if we already have the account
        if let Some(account) = self.accounts.get(&address) {
            return Ok(account.info());
        }

        // fetch the account data if we dont have the account and insert it into database
        let account_info = <Self as DatabaseRef>::basic_ref(self, address)?;
        let account = match account_info {
            Some(info) => BlockStateDBAccount {info, ..Default::default()},
            None => BlockStateDBAccount::new_not_existing()
        };
        self.accounts.insert(address, account.clone());
        Ok(account.info())
    }

    fn code_by_hash(&mut self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        println!("looking for {:?}", code_hash);
        if let Some(code) = self.contracts.get(&code_hash) {
            return Ok(code.clone());
        }
        
        let bytecode = <Self as DatabaseRef>::code_by_hash_ref(self, code_hash)?;
        self.contracts.insert(code_hash, bytecode.clone());
        Ok(bytecode)
    }

    // update all account storage slots for an account
    #[inline]
    fn storage(&mut self, address: Address, index: U256) -> Result<U256, Self::Error> {
        // Check if the account exists
        if let Some(account) = self.accounts.get_mut(&address) {
            // Check if the storage slot exists
            if let Some(value) = account.storage.get(&index) {
                return Ok(*value);
            }

            // Check the account state
            if matches!(account.state, AccountState::StorageCleared | AccountState::NotExisting) {
                return Ok(U256::ZERO);
            }

            // Drop the mutable borrow before calling storage_ref
            // by ending the scope
        }

        // Now it's safe to call storage_ref with an immutable borrow of self
        let slot_value = <Self as DatabaseRef>::storage_ref(self, address, index)?;
        
        // Re-obtain the mutable reference to update the storage
        if let Some(account) = self.accounts.get_mut(&address) {
            account.storage.insert(index, slot_value);
            return Ok(slot_value);
        }

        // create a default account
        let new_account = BlockStateDBAccount::new_not_existing();
        self.accounts.insert(address, new_account);
        self.insert_account_storage(address, index, slot_value).unwrap();

        // Handle the case where the account might have been removed in between
        Ok(slot_value)
    }

    fn block_hash(&mut self, number: BlockNumber) -> Result<B256, Self::Error> {
        if let Some(hash) = self.block_hashes.get(&number) {
            return Ok(*hash);
        }

        let hash = <Self as DatabaseRef>::block_hash_ref(self, number)?;
        self.block_hashes.insert(number, hash);
        Ok(hash)
    }
}


// Implement required database ref trait
impl<T: Transport + Clone, N: Network, P: Provider<T, N>> DatabaseRef for BlockStateDB<T, N, P> {
    type Error = TransportError;
    
    fn basic_ref(&self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        println!("looking for ref {:?}", address);
        match self.accounts.get(&address) {
            Some(acc) => Ok(acc.info()),
            None => {
                let f = async {
                    let nonce = self
                        .provider
                        .get_transaction_count(address)
                        .block_id(BlockId::latest());
                    let balance = self
                        .provider
                        .get_balance(address)
                        .block_id(BlockId::latest());
                    let code = self
                        .provider
                        .get_code_at(address)
                        .block_id(BlockId::latest());
                    tokio::join!(
                        nonce,
                        balance,
                        code
                    )
                };
                let (nonce, balance, code) = self.runtime.block_on(f);
                let balance = balance?;
                let code = Bytecode::new_raw(code?.0.into());
                let code_hash = code.hash_slow();
                let nonce = nonce?;
        
                Ok(Some(AccountInfo::new(balance, nonce, code_hash, code)))
            }
        }
    }

    fn code_by_hash_ref(&self, code_hash:B256) -> Result<Bytecode,Self::Error> {
        match self.contracts.get(&code_hash) {
            Some(entry) => Ok(entry.clone()),
            None => {
                panic!("the codehsould already be loaded");
            }
        }
    }

    fn storage_ref(&self,address:Address,index:U256) -> Result<U256,Self::Error> {
        println!("looking ref asdfasdfasdasdffor {:?}, {:?}", address, index);
        match self.accounts.get(&address) {
            Some(acc_entry) => match acc_entry.storage.get(&index) {
                Some(entry) => Ok(*entry),
                None => {
                    if matches!(
                        acc_entry.state,
                        AccountState::StorageCleared | AccountState::NotExisting
                    ) {
                        println!("running asdfhere");
                        Ok(U256::ZERO)
                    } else {
                        println!("running here");
                        let f = self.provider.get_storage_at(address, index);
                        let slot_val = self.runtime.block_on(f.into_future())?;
                        Ok(slot_val)
                    }
                }
            },
            None => {
                println!("{}, {}", address, index);
                let f = self.provider.get_storage_at(address, index);
                let slot_val = self.runtime.block_on(f.into_future())?;
                println!("{:?}", slot_val);
                Ok(slot_val)
            }
        }
    }

    fn block_hash_ref(&self,number: BlockNumber) -> Result<B256,Self::Error> {
        match self.block_hashes.get(&number) {
            Some(entry) => Ok(*entry),
            None => {
                let block = self.runtime.block_on(
                    self.provider
                        .get_block_by_number(number.into(), false),
                )?;
                Ok(B256::new(*block.unwrap().header().hash()))
            }
        }
    }
}



#[derive(Default, Debug, Clone)]
pub struct BlockStateDBAccount {
    pub info: AccountInfo,
    pub state: AccountState,
    pub storage: HashMap<U256, U256>
}

impl BlockStateDBAccount {
    pub fn new_not_existing() -> Self {
        Self {
            state: AccountState::NotExisting,
            ..Default::default()
        }
    }

    pub fn info(&self) -> Option<AccountInfo> {
        if matches!(self.state, AccountState::NotExisting) {
            None
        } else {
            Some(self.info.clone())
        }
    }
}

impl From<Option<AccountInfo>> for BlockStateDBAccount {
    fn from(from: Option<AccountInfo>) -> Self {
        from.map(Self::from).unwrap_or_else(Self::new_not_existing)
    }
}

impl From<AccountInfo> for BlockStateDBAccount {
    fn from(info: AccountInfo) -> Self {
        Self {
            info,
            state: AccountState::None,
            ..Default::default()
        }
    }
}



































