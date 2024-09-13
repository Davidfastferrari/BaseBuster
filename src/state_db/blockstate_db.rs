
use alloy::primitives::{Address, U256, B256, BlockNumber};

use revm::primitives::{Account, AccountInfo, Bytecode, Log, KECCAK_EMPTY};
use revm::db::AccountState;
use revm::{Database, DatabaseCommit, DatabaseRef};
use std::collections::HashMap;
use std::collections::hash_map::Entry;

pub struct BlockStateDB<ExtDB> {
    pub accounts: HashMap<Address, BlockStateDBAccount>,
    pub contracts: HashMap<B256, Bytecode>, 
    pub logs: Vec<Log>,
    pub block_hashes: HashMap<BlockNumber, B256>,
    pub db: ExtDB

}

impl<ExtDB: Default> Default for BlockStateDB<ExtDB> {
    fn default() -> Self {
        Self::new(ExtDB::default())
    }
}

impl<ExtDB> BlockStateDB<ExtDB> {
    // Construct a new BlockStateDB
    pub fn new(db: ExtDB) -> Self {
        let mut contracts = HashMap::new();
        contracts.insert(KECCAK_EMPTY, Bytecode::default());
        contracts.insert(B256::ZERO, Bytecode::default());
        Self {
            accounts: HashMap::new(),
            contracts,
            logs: Vec::new(),
            block_hashes: HashMap::new(),
            db
        }
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
}


// Implement functions when ExtDB implements Database ref
impl<ExtDB: DatabaseRef> BlockStateDB<ExtDB> {
    pub fn load_account(&mut self, address: Address) -> Result<&mut BlockStateDBAccount, ExtDB::Error> {
        let db = &self.db;
        match self.accounts.entry(address) {
            Entry::Occupied(entry) => Ok(entry.into_mut()),
            Entry::Vacant(entry) => Ok(entry.insert(
                db.basic_ref(address)?
                    .map(|info| BlockStateDBAccount { info, ..Default::default() })
                    .unwrap_or_else(BlockStateDBAccount::new_not_existing),
            )),
        }
    }

    /// insert account storage without overriding account info
    pub fn insert_account_storage(&mut self, address: Address, slot: U256, value: U256) -> Result<(), ExtDB::Error> {
        let account = self.load_account(address)?;
        account.storage.insert(slot, value);
        Ok(())
    }

    /// replace account storage without overriding account info
    pub fn replace_account_storage(&mut self, address: Address, storage: HashMap<U256, U256>) -> Result<(), ExtDB::Error> {
        let account = self.load_account(address)?;
        account.state = AccountState::StorageCleared;
        account.storage = storage.into_iter().collect();
        Ok(())
    }

}

impl<ExtDB> DatabaseCommit for BlockStateDB<ExtDB> {
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
            db_account.storage.extend(account.storage.into_iter().map(|(key, value)| (key, value.present_value())));
        }
    }
}

impl<ExtDB: DatabaseRef> Database for BlockStateDB<ExtDB> {
    type Error = ExtDB::Error;

    fn basic(&mut self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        let basic = match self.accounts.entry(address) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(
                self.db
                    .basic_ref(address)?
                    .map(|info| BlockStateDBAccount { info, ..Default::default() })
                    .unwrap_or_else(BlockStateDBAccount::new_not_existing),
            ),
        };
        Ok(basic.info())
    }

    fn code_by_hash(&mut self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        match self.contracts.entry(code_hash) {
            Entry::Occupied(entry) => Ok(entry.get().clone()),
            Entry::Vacant(entry) => {
                // if you return code bytes when basic fn is called this function is not needed.
                Ok(entry.insert(self.db.code_by_hash_ref(code_hash)?).clone())
            }
        }
    }

    /// Get the value in an account's storage slot.
    ///
    /// It is assumed that account is already loaded.
    fn storage(&mut self, address: Address, index: U256) -> Result<U256, Self::Error> {
        match self.accounts.entry(address) {
            Entry::Occupied(mut acc_entry) => {
                let acc_entry = acc_entry.get_mut();
                match acc_entry.storage.entry(index) {
                    Entry::Occupied(entry) => Ok(*entry.get()),
                    Entry::Vacant(entry) => {
                        if matches!(acc_entry.state, AccountState::StorageCleared | AccountState::NotExisting) {
                            Ok(U256::ZERO)
                        } else {
                            let slot = self.db.storage_ref(address, index)?;
                            entry.insert(slot);
                            Ok(slot)
                        }
                    }
                }
            }
            Entry::Vacant(acc_entry) => {
                // acc needs to be loaded for us to access slots.
                let info = self.db.basic_ref(address)?;
                let (account, value) = if info.is_some() {
                    let value = self.db.storage_ref(address, index)?;
                    let mut account: BlockStateDBAccount = info.into();
                    account.storage.insert(index, value);
                    (account, value)
                } else {
                    (info.into(), U256::ZERO)
                };
                acc_entry.insert(account);
                Ok(value)
            }
        }
    }

    fn block_hash(&mut self, number: BlockNumber) -> Result<B256, Self::Error> {
        match self.block_hashes.entry(number) {
            Entry::Occupied(entry) => Ok(*entry.get()),
            Entry::Vacant(entry) => {
                let hash = self.db.block_hash_ref(number)?;
                entry.insert(hash);
                Ok(hash)
            }
        }
    }
}


// Implement Database Ref for our db
impl<ExtDB: DatabaseRef> DatabaseRef for BlockStateDB<ExtDB> {
    type Error = ExtDB::Error;
    
    fn basic_ref(&self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        match self.accounts.get(&address) {
            Some(acc) => Ok(acc.info()),
            None => self.db.basic_ref(address)
        }
    }

    fn code_by_hash_ref(&self, code_hash:B256) -> Result<Bytecode,Self::Error> {
        match self.contracts.get(&code_hash) {
            Some(entry) => Ok(entry.clone()),
            None => self.db.code_by_hash_ref(code_hash)
        }
    }

    fn storage_ref(&self,address:Address,index:U256) -> Result<U256,Self::Error> {
        match self.accounts.get(&address) {
            Some(acc_entry) => match acc_entry.storage.get(&index) {
                Some(entry) => Ok(*entry),
                None => {
                    if matches!(
                        acc_entry.state,
                        AccountState::StorageCleared | AccountState::NotExisting
                    ) {
                        Ok(U256::ZERO)
                    } else {
                        self.db.storage_ref(address, index)
                    }
                }
            },
            None => self.db.storage_ref(address, index),
        }
    }

    fn block_hash_ref(&self,number: BlockNumber) -> Result<B256,Self::Error> {
        match self.block_hashes.get(&number) {
            Some(entry) => Ok(*entry),
            None => self.db.block_hash_ref(number),
        }
    }
}



#[derive(Default)]
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



































