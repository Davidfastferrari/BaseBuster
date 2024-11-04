use alloy::network::{BlockResponse, HeaderResponse, Network};
use alloy::primitives::{Address, BlockNumber, B256, U256};
use alloy::providers::Provider;
use alloy::rpc::types::trace::geth::AccountState as GethAccountState;
use alloy::rpc::types::BlockId;
use alloy::transports::{Transport, TransportError};
use anyhow::Result;
use log::{debug, info, trace, warn};
use pool_sync::PoolType;
use revm::primitives::{Log, KECCAK_EMPTY};
use revm::state::{Account, AccountInfo, Bytecode};
use revm::{Database, DatabaseCommit, DatabaseRef};
use revm_database::AccountState;
use std::collections::HashMap;
use std::collections::HashSet;
use std::future::IntoFuture;
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
    // All of the accounts
    pub accounts: HashMap<Address, BlockStateDBAccount>,

    // The contracts that this database holds
    pub contracts: HashMap<B256, Bytecode>,
    // Logs??
    pub logs: Vec<Log>,
    // Block hashs???
    pub block_hashes: HashMap<BlockNumber, B256>,

    // The pools that are in our working set
    pub pools: HashSet<Address>,
    //
    pub pool_info: HashMap<Address, PoolInformation>,
    // provider for fetching information
    provider: P,
    runtime: HandleOrRuntime,
    _marker: std::marker::PhantomData<fn() -> (T, N)>,
}

impl<T: Transport + Clone, N: Network, P: Provider<T, N>> BlockStateDB<T, N, P> {
    // Construct a new BlockStateDB
    pub fn new(provider: P) -> Option<Self> {
        debug!("Creating new BlockStateDB");
        let mut contracts = HashMap::new();
        contracts.insert(KECCAK_EMPTY, Bytecode::default());
        contracts.insert(B256::ZERO, Bytecode::default());

        // get our runtime handle
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
            _marker: std::marker::PhantomData,
        })
    }

    // Record a new pool in our working set
    pub fn add_pool(
        &mut self,
        pool: Address,
        token0: Address,
        token1: Address,
        pool_type: PoolType,
    ) {
        trace!("Adding pool {} to database", pool);
        self.pools.insert(pool);
        self.pool_info.insert(
            pool,
            PoolInformation {
                token0,
                token1,
                pool_type,
            },
        );
        let pool_account = BlockStateDBAccount::new_not_existing();
        self.accounts.insert(pool, pool_account);
    }

    // Check if we are tracking the pool. This is our working set
    #[inline]
    pub fn tracking_pool(&self, pool: &Address) -> bool {
        self.pools.contains(pool)
    }

    // Compute zero to one for amount out computations
    #[inline]
    pub fn zero_to_one(&self, pool: &Address, token_in: Address) -> Option<bool> {
        self.pool_info.get(pool).map(|info| info.token0 == token_in)
    }

    // Go through a block trace and update all relevant slots
    #[inline]
    pub fn update_all_slots(
        &mut self,
        address: Address,
        account_state: GethAccountState,
    ) -> Result<()> {
        trace!(
            "Update all slots: updating all storage slots for adddress {}",
            address
        );
        let storage = account_state.storage;
        for (slot, value) in storage {
            if let Some(account) = self.accounts.get_mut(&address) {
                account.storage.insert(slot.into(), value.into());
            }
        }
        Ok(())
    }
}

// Implement the database trait for the BlockStateDB
impl<T: Transport + Clone, N: Network, P: Provider<T, N>> Database for BlockStateDB<T, N, P> {
    type Error = TransportError;

    // Get basic account information
    fn basic(&mut self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        trace!("Database Basic: Looking for account {}", address);
        // Look if we already have the account
        if let Some(account) = self.accounts.get(&address) {
            trace!("Database Basic: Account {} found in database", address);
            return Ok(account.info());
        }

        // Fetch the account data if we don't have the account and insert it into database
        trace!(
            "Database Basic: Account {} not found in cache. Fetching info via basic_ref",
            address
        );
        let account_info = <Self as DatabaseRef>::basic_ref(self, address)?;
        let account = match account_info {
            Some(info) => {
                trace!("Database Basic: Account {} fetched from basic_ref", address);
                BlockStateDBAccount {
                    info,
                    ..Default::default()
                }
            }
            None => {
                trace!(
                    "Database Basic: Unable to fetch account {} from basic_ref",
                    address
                );
                BlockStateDBAccount::new_not_existing()
            }
        };
        self.accounts.insert(address, account.clone());
        trace!("Database Basic: Inserted account {} into database", address);
        Ok(account.info())
    }

    // Get account code by its hash
    fn code_by_hash(&mut self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        trace!(
            "Database Code By Hash: Fetching code for hash {}",
            code_hash
        );
        // Look if we already have the code
        if let Some(code) = self.contracts.get(&code_hash) {
            trace!(
                "Database Code By Hash: Code for hash {} found in database",
                code_hash
            );
            return Ok(code.clone());
        }

        trace!("Database Code By Hash: Code for hash {} not found in cache. Fetching code via code_by_hash_ref", code_hash);
        let bytecode = <Self as DatabaseRef>::code_by_hash_ref(self, code_hash);
        let bytecode = match bytecode {
            Ok(bytecode) => {
                trace!(
                    "Database Code By Hash: Code for hash {} fetched from code_by_hash_ref",
                    code_hash
                );
                bytecode
            }
            Err(_) => {
                trace!(
                    "Database Code By Hash: Unable to fetch code for hash {} from code_by_hash_ref",
                    code_hash
                );
                Bytecode::new()
            }
        };

        self.contracts.insert(code_hash, bytecode.clone());
        trace!(
            "Database Code By Hash: Inserted code for hash {} into database",
            code_hash
        );
        Ok(bytecode)
    }

    // Get storage value of address at index
    fn storage(&mut self, address: Address, index: U256) -> Result<U256, Self::Error> {
        trace!(
            "Database Storage: Fetching storage for address {}, slot {}",
            address,
            index
        );

        // Check if the account exists
        if let Some(account) = self.accounts.get(&address) {
            // Check if the storage slot exists
            if let Some(value) = account.storage.get(&index) {
                trace!(
                    "Database Storage: Storage for address {}, slot {} found in database",
                    address,
                    index
                );
                return Ok(*value);
            }

            // The account exists, but the storage slot does not, fetch it
            trace!("Database Storage: Account {} found, but slot {} missing. Fetching slot via storage_ref", address, index);
            // todo!() make sure this is valid
            let value = <Self as DatabaseRef>::storage_ref(self, address, index)?;

            trace!(
                "Database Storage: Fetched slot {} for account {}. Inserting storage into database",
                index,
                address
            );

            // get a mutable ref to the account and insert the storage value in
            let account = self.accounts.get_mut(&address).unwrap();
            account.storage.insert(index, value);
            return Ok(value);
        }

        // This is a brand new account, fetch the slots and make/insert a new account
        trace!(
            "Database Storage: Account {} not found in database. Fetching account and slot info",
            address
        );

        // insert account via basic(), retrieve account and fetch storage value, insert storage
        // value
        self.basic(address)?; // this will fetch/insert the account
        let slot_value = <Self as DatabaseRef>::storage_ref(self, address, index)?;
        let account = self.accounts.get_mut(&address).unwrap();
        account.storage.insert(index, slot_value);
        trace!(
            "Database Storage: Inserted account {} and slot {} into database",
            address,
            index
        );

        Ok(slot_value)
    }

    fn block_hash(&mut self, number: BlockNumber) -> Result<B256, Self::Error> {
        // todo!(), thisisnt really used but should make it better
        debug!("Fetching block hash for block number: {:?}", number);
        if let Some(hash) = self.block_hashes.get(&number) {
            debug!(
                "Block hash found in database for block number: {:?}",
                number
            );
            return Ok(*hash);
        }

        debug!(
            "Block hash not found in cache, fetching from provider for block number: {:?}",
            number
        );
        let hash = <Self as DatabaseRef>::block_hash_ref(self, number)?;
        self.block_hashes.insert(number, hash);
        Ok(hash)
    }
}

// Implement required DatabaseRef trait, read references to the database (fetch from provider)
impl<T: Transport + Clone, N: Network, P: Provider<T, N>> DatabaseRef for BlockStateDB<T, N, P> {
    type Error = TransportError;

    // Get basic account information
    fn basic_ref(&self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        trace!("Database BasicRef: Looking for account {}", address);

        // look if we already have the account
        if let Some(account) = self.accounts.get(&address) {
            trace!("Database Basic: Account {} found in cache", address);
            return Ok(account.info());
        }

        // we do not have the account, fetch from the provider
        trace!(
            "Database BasicRef: Account {} not found in cache. Fetching info from provider",
            address
        );
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
            tokio::join!(nonce, balance, code)
        };
        let (nonce, balance, code) = self.runtime.block_on(f);

        match (nonce, balance, code) {
            (Ok(nonce_val), Ok(balance_val), Ok(code_val)) => {
                trace!(
                    "Database BasicRef: Fetched account {} from provider",
                    address
                );
                let balance = balance_val;
                let code = Bytecode::new_raw(code_val.0.into());
                let code_hash = code.hash_slow();
                let nonce = nonce_val;

                Ok(Some(AccountInfo::new(balance, nonce, code_hash, code)))
            }
            _ => {
                trace!(
                    "Database BasicRef: Unable to fetch account {} from provider",
                    address
                );
                Ok(None)
            }
        }
    }

    // Get account code by its hash
    fn code_by_hash_ref(&self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        trace!(
            "Database Code By Hash Ref: Fetching code for hash {}",
            code_hash
        );
        // Look if we already have the code
        if let Some(code) = self.contracts.get(&code_hash) {
            trace!(
                "Database Code By Hash Ref: Code for hash {} found in cache",
                code_hash
            );
            return Ok(code.clone());
        }

        // the code should already be loaded??
        panic!("The code should already be loaded");
    }

    // Get storage value of address at index
    fn storage_ref(&self, address: Address, index: U256) -> Result<U256, Self::Error> {
        trace!(
            "Database Storage Ref: Fetching storage for address {}, slot {}",
            address,
            index
        );

        if let Some(account) = self.accounts.get(&address) {
            if let Some(value) = account.storage.get(&index) {
                trace!(
                    "Database Storage Ref: Storage for address {}, slot {} found in database",
                    address,
                    index
                );
                return Ok(*value);
            }
        }
        trace!(
            "Database Storage Ref: Account {} not found. Fetching slot {} from provider",
            address,
            index
        );
        let f = self.provider.get_storage_at(address, index);
        let slot_val = self.runtime.block_on(f.into_future())?;
        Ok(slot_val)
    }

    fn block_hash_ref(&self, number: BlockNumber) -> Result<B256, Self::Error> {
        debug!("Fetching block_hash_ref for block number: {:?}", number);
        match self.block_hashes.get(&number) {
            Some(entry) => {
                debug!(
                    "Block hash found in cache for block number: {:?}, hash: {:?}",
                    number, entry
                );
                Ok(*entry)
            }
            None => {
                debug!(
                    "Block hash not found in cache, fetching from provider for block number: {:?}",
                    number
                );
                let block = self
                    .runtime
                    .block_on(self.provider.get_block_by_number(number.into(), false))?;
                match block {
                    Some(block_data) => {
                        let hash = B256::new(*block_data.header().hash());
                        debug!(
                            "Fetched block hash from provider for block number: {:?}, hash: {:?}",
                            number, hash
                        );
                        Ok(hash)
                    }
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

            if let Some(code) = &account.info.code {
                if !code.is_empty() {
                    account.info.code_hash = code.hash_slow();
                    if self
                        .contracts
                        .insert(account.info.code_hash, code.clone())
                        .is_some()
                    {
                        trace!(
                            "Updated existing contract with hash: {:?}",
                            account.info.code_hash
                        );
                    } else {
                        trace!(
                            "Inserted new contract with hash: {:?}",
                            account.info.code_hash
                        );
                    }
                    return;
                }
            } else {
                account.info.code_hash = KECCAK_EMPTY;
            }

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
    pub storage: HashMap<U256, U256>,
}

impl BlockStateDBAccount {
    pub fn new_not_existing() -> Self {
        trace!("Creating a new non-existing BlockStateDBAccount");
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
