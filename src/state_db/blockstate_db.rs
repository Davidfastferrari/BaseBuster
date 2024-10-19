use alloy::primitives::{Address, U256, B256, BlockNumber};
use revm::state::{Account, AccountInfo, Bytecode};
use revm::primitives::{Log, KECCAK_EMPTY};
use revm_database::AccountState;

use alloy::primitives::address;
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
use anyhow::Result;
use log::{debug, info, warn, error};
use tokio::runtime::{Handle, Runtime};


#[derive(Debug)]
pub enum HandleOrRuntime {
    Handle(Handle),
    Runtime(Runtime),
}

impl HandleOrRuntime {
    #[inline]
    pub fn block_on<F>(&self, f: F) -> F::Output
    where
        F: std::future::Future + Send,
        F::Output: Send,
    {
        match self {
            Self::Handle(handle) => tokio::task::block_in_place(move || handle.block_on(f)),
            Self::Runtime(rt) => rt.block_on(f),
        }
    }
}

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
    runtime: HandleOrRuntime,
    _marker: std::marker::PhantomData<fn() -> (T, N)>,
}

impl<T: Transport + Clone, N: Network, P: Provider<T, N>> BlockStateDB<T, N, P> {
    // Construct a new BlockStateDB
    pub fn new(provider: P) -> Option<Self> {
        debug!("Initializing new BlockStateDB");
        let mut contracts = HashMap::new();
        contracts.insert(KECCAK_EMPTY, Bytecode::default());
        contracts.insert(B256::ZERO, Bytecode::default());

        let rt = match Handle::try_current() {
            Ok(handle) => match handle.runtime_flavor() {
                tokio::runtime::RuntimeFlavor::CurrentThread => return None,
                _ => HandleOrRuntime::Handle(handle),
            },
            Err(_) => return None,
        };


        Some(Self {
            accounts: HashMap::new(),
            contracts,
            logs: Vec::new(),
            block_hashes: HashMap::new(),
            pools: HashSet::new(),
            pool_info: HashMap::new(),
            provider,
            runtime: rt,
            _marker: std::marker::PhantomData
        })
    }

    // Track pool information for easy access
    pub fn add_pool(&mut self, pool: Address, token0: Address, token1: Address, pool_type: PoolType) {
        self.pools.insert(pool);
        self.pool_info.insert(pool, PoolInformation { token0, token1, pool_type });
    }

    // Insert a contract into the DB
    pub fn insert_contract(&mut self, account: &mut AccountInfo) {
        debug!("Inserting contract for account: {:?}", account);
        if let Some(code) = &account.code {
            if !code.is_empty() {
                if account.code_hash == KECCAK_EMPTY {
                    debug!("Updating code_hash for account: {:?}", account);
                    account.code_hash = code.hash_slow();
                }
                if self
                    .contracts
                    .insert(account.code_hash, code.clone())
                    .is_none()
                {
                    info!("Inserted new contract with hash: {:?}", account.code_hash);
                } else {
                    debug!(
                        "Contract with hash {:?} already exists, skipping insert",
                        account.code_hash
                    );
                }
            }
        }
        if account.code_hash.is_zero() {
            debug!("Code hash is zero, setting to KECCAK_EMPTY");
            account.code_hash = KECCAK_EMPTY;
        }
    }

    // Insert some account info into the DB
    pub fn insert_account_info(&mut self, address: Address, mut info: AccountInfo) {
        debug!("Inserting account info for address: {:?}", address);
        self.insert_contract(&mut info);
        self.accounts.entry(address).or_default().info = info;
    }

    pub fn load_account(&mut self, address: Address) -> Result<&mut BlockStateDBAccount> {
        match self.accounts.entry(address) {
            Entry::Occupied(entry) => {
                debug!("Loading existing account for address: {:?}", address);
                Ok(entry.into_mut())
            }
            Entry::Vacant(entry) => {
                debug!("Account not found for address: {:?}, inserting new account", address);
                Ok(entry.insert(
                    BlockStateDBAccount::new_not_existing()
                ))
            }
        }
    }

    /// Insert account storage without overriding account info
    #[inline]
    pub fn insert_account_storage(&mut self, address: Address, slot: U256, value: U256) -> Result<()> {
        debug!(
            "Inserting storage for address: {:?}, slot: {:?}, value: {:?}",
            address, slot, value
        );
        let account = self.load_account(address)?;
        account.storage.insert(slot, value);
        Ok(())
    }


    // Update all account storage slots for an account
    #[inline]
    pub fn update_all_slots(&mut self, address: Address, account_state: GethAccountState) -> Result<()> {
        debug!(
            "Updating all storage slots for address: {:?}, account_state: {:?}",
            address, account_state
        );
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
        debug!("Fetching basic account info for address: {:?}", address);
        // Look if we already have the account
        if let Some(account) = self.accounts.get(&address) {
            debug!("Account found in cache for address: {:?}", address);
            return Ok(account.info());
        }

        // Fetch the account data if we don't have the account and insert it into database
        debug!("Account not found in cache, fetching from provider for address: {:?}", address);
        let account_info = <Self as DatabaseRef>::basic_ref(self, address)?;
        let account = match account_info {
            Some(info) => {
                debug!("Fetched account info from provider for address: {:?}", address);
                BlockStateDBAccount {info, ..Default::default()}
            },
            None => {
                debug!("No account info found for address: {:?}", address);
                BlockStateDBAccount::new_not_existing()
            }
        };
        self.accounts.insert(address, account.clone());
        Ok(account.info())
    }

    fn code_by_hash(&mut self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        debug!("Fetching code by hash: {:?}", code_hash);
        if let Some(code) = self.contracts.get(&code_hash) {
            debug!("Code found in cache for hash: {:?}", code_hash);
            return Ok(code.clone());
        }
        
        debug!("Code not found in cache, fetching from provider for hash: {:?}", code_hash);
        let bytecode = <Self as DatabaseRef>::code_by_hash_ref(self, code_hash)?;
        self.contracts.insert(code_hash, bytecode.clone());
        Ok(bytecode)
    }

    // Update all account storage slots for an account
    #[inline]
    fn storage(&mut self, address: Address, index: U256) -> Result<U256, Self::Error> {
        debug!("Accessing storage for address: {:?}, index: {:?}", address, index);
        // Check if the account exists
        if let Some(account) = self.accounts.get_mut(&address) {
            // Check if the storage slot exists
            if let Some(value) = account.storage.get(&index) {
                debug!(
                    "Storage slot found in cache for address: {:?}, index: {:?}",
                    address, index
                );
                return Ok(*value);
            }

            // The account exists, but we do not have the slot
            // Fetch, insert, return
            debug!(
                "Storage slot not found in cache, fetching from provider for address: {:?}, index: {:?}",
                address, index
            );
            let value = <Self as DatabaseRef>::storage_ref(self, address, index)?;
            self.insert_account_storage(address, index, value).unwrap(); // fix error
            return Ok(value);
        }

        // This is a brand new account, fetch the slots and make/insert a new account
        debug!(
            "Account not found in cache, fetching storage for new account: {:?}, index: {:?}",
            address, index
        );
        let slot_value = <Self as DatabaseRef>::storage_ref(self, address, index)?;
        let new_account = BlockStateDBAccount::new_not_existing();
        self.accounts.insert(address, new_account);
        self.insert_account_storage(address, index, slot_value).unwrap();

        // Handle the case where the account might have been removed in between
        debug!(
            "Fetched and inserted storage slot for address: {:?}, index: {:?}",
            address, index
        );
        Ok(slot_value)
    }

    fn block_hash(&mut self, number: BlockNumber) -> Result<B256, Self::Error> {
        debug!("Fetching block hash for block number: {:?}", number);
        if let Some(hash) = self.block_hashes.get(&number) {
            debug!("Block hash found in cache for block number: {:?}", number);
            return Ok(*hash);
        }

        debug!("Block hash not found in cache, fetching from provider for block number: {:?}", number);
        let hash = <Self as DatabaseRef>::block_hash_ref(self, number)?;
        self.block_hashes.insert(number, hash);
        Ok(hash)
    }
}




// Implement required DatabaseRef trait
impl<T: Transport + Clone, N: Network, P: Provider<T, N>> DatabaseRef for BlockStateDB<T, N, P> {
    type Error = TransportError;
    
    fn basic_ref(&self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        debug!("Fetching basic_ref for address: {:?}", address);
        match self.accounts.get(&address) {
            Some(acc) => {
                debug!("Account info found in cache for address: {:?}", address);
                Ok(acc.info())
            },
            None => {
                debug!("Account info not found in cache, fetching from provider for address: {:?}", address);
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
                match (nonce, balance, code) {
                    (Ok(nonce_val), Ok(balance_val), Ok(code_val)) => {
                        debug!(
                            "Fetched from provider - nonce: {:?}, balance: {:?}, code: {:?}",
                            nonce_val, balance_val, code_val
                        );
                        let balance = balance_val;
                        let code = Bytecode::new_raw(code_val.0.into());
                        let code_hash = code.hash_slow();
                        let nonce = nonce_val;
                
                        Ok(Some(AccountInfo::new(balance, nonce, code_hash, code)))
                    },
                    _ => {
                        warn!("Failed to fetch account info from provider for address: {:?}", address);
                        Ok(None)
                    }
                }
            }
        }
    }

    fn code_by_hash_ref(&self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        debug!("Fetching code_by_hash_ref for hash: {:?}", code_hash);
        match self.contracts.get(&code_hash) {
            Some(entry) => {
                debug!("Code found in cache for hash: {:?}", code_hash);
                Ok(entry.clone())
            },
            None => {
                error!("Code with hash {:?} not found in cache and cannot be fetched", code_hash);
                panic!("The code should already be loaded");
            }
        }
    }

    fn storage_ref(&self,address:Address,index:U256) -> Result<U256,Self::Error> {
        debug!("Fetching storage_ref for address: {:?}, index: {:?}", address, index);
        match self.accounts.get(&address) {
            Some(acc_entry) => match acc_entry.storage.get(&index) {
                Some(entry) => {
                    debug!(
                        "Storage slot found in cache for address: {:?}, index: {:?}",
                        address, index
                    );
                    Ok(*entry)
                },
                None => {
                    debug!(
                        "Storage slot not found in cache, fetching from provider for address: {:?}, index: {:?}",
                        address, index
                    );
                    let f = self.provider.get_storage_at(address, index);
                    let slot_val = self.runtime.block_on(f.into_future())?;
                    debug!(
                        "Fetched storage value from provider for address: {:?}, index: {:?}, value: {:?}",
                        address, index, slot_val
                    );
                    Ok(slot_val)
                }
            },
            None => {
                debug!(
                    "Account not found in cache, fetching storage from provider for address: {:?}, index: {:?}",
                    address, index
                );
                let f = self.provider.get_storage_at(address, index);
                let slot_val = self.runtime.block_on(f.into_future())?;
                debug!(
                    "Fetched storage value from provider for address: {:?}, index: {:?}, value: {:?}",
                    address, index, slot_val
                );
                Ok(slot_val)
            }
        }
    }

    fn block_hash_ref(&self,number: BlockNumber) -> Result<B256,Self::Error> {
        debug!("Fetching block_hash_ref for block number: {:?}", number);
        match self.block_hashes.get(&number) {
            Some(entry) => {
                debug!(
                    "Block hash found in cache for block number: {:?}, hash: {:?}",
                    number, entry
                );
                Ok(*entry)
            },
            None => {
                debug!(
                    "Block hash not found in cache, fetching from provider for block number: {:?}",
                    number
                );
                let block = self.runtime.block_on(
                    self.provider
                        .get_block_by_number(number.into(), false),
                )?;
                match block {
                    Some(block_data) => {
                        let hash = B256::new(*block_data.header().hash());
                        debug!(
                            "Fetched block hash from provider for block number: {:?}, hash: {:?}",
                            number, hash
                        );
                        Ok(hash)
                    },
                    None => {
                        warn!("No block found for block number: {:?}", number);
                        Ok(B256::ZERO)
                    }
                }
            }
        }
    }
}


impl<T: Transport + Clone, N: Network, P: Provider<T, N>> DatabaseCommit for BlockStateDB<T, N, P> {
    fn commit(&mut self, changes: HashMap<Address, Account>) {
        for (address, mut account) in changes {
            if !account.is_touched() {
                continue;
            }
            if account.is_selfdestructed() {
                let db_account = self.accounts.entry(address).or_default();
                db_account.storage.clear();
                db_account.state = AccountState::NotExisting;
                db_account.info = AccountInfo::default();
                continue;
            }
            let is_newly_created = account.is_created();
            self.insert_contract(&mut account.info);

            let db_account = self.accounts.entry(address).or_default();
            db_account.info = account.info;

            db_account.state = if is_newly_created {
                db_account.storage.clear();
                AccountState::StorageCleared
            } else if db_account.state.is_storage_cleared() {
                // Preserve old account state if it already exists
                AccountState::StorageCleared
            } else {
                AccountState::Touched
            };
            db_account.storage.extend(
                account
                    .storage
                    .into_iter()
                    .map(|(key, value)| (key, value.present_value())),
            );
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
        debug!("Creating a new non-existing BlockStateDBAccount");
        Self {
            state: AccountState::NotExisting,
            ..Default::default()
        }
    }

    pub fn info(&self) -> Option<AccountInfo> {
        if matches!(self.state, AccountState::NotExisting) {
            debug!("AccountState is NotExisting, returning None");
            None
        } else {
            debug!("Returning account info: {:?}", self.info);
            Some(self.info.clone())
        }
    }
}

impl From<Option<AccountInfo>> for BlockStateDBAccount {
    fn from(from: Option<AccountInfo>) -> Self {
        match from {
            Some(info) => {
                debug!("Converting Some(AccountInfo) into BlockStateDBAccount");
                Self::from(info)
            },
            None => {
                debug!("Converting None into BlockStateDBAccount::new_not_existing");
                Self::new_not_existing()
            }
        }
    }
}

impl From<AccountInfo> for BlockStateDBAccount {
    fn from(info: AccountInfo) -> Self {
        debug!("Converting AccountInfo into BlockStateDBAccount");
        Self {
            info,
            state: AccountState::None,
            ..Default::default()
        }
    }
}



#[cfg(test)]
mod BlockStateDB_TESTS {
}


































