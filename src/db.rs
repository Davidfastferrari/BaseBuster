use reth::api::NodeTypesWithDBAdapter;
use reth_primitives::StorageKey;
use reth_provider::ProviderFactory;
use std::path::Path;
use reth_db::open_db_read_only;
use reth_db::mdbx::{DatabaseArguments, DatabaseEnvKind};
use reth_db::models::ClientVersion;
use reth_provider::providers::StaticFileProvider;
use reth_chainspec::{ChainSpecBuilder, Chain};
use reth_node_ethereum::EthereumNode;
use reth_optimism_chainspec::BASE_MAINNET;
use reth_provider::StateProviderBox;
use revm::{Database, DatabaseRef, interpreter};
use revm::primitives::{AccountInfo, Address, B256, Bytecode, KECCAK_EMPTY, U256};
use anyhow::Error;
use std::sync::Arc;
use std::sync::Mutex;

        use reth_db::DatabaseEnv;

type Lock = Arc<Mutex<()>>;

pub struct RethDB {
    provider: StateProviderBox
}

impl RethDB {
    pub fn new() -> Self {
        let db_path = Path::new("/home/ubuntu/base-docker/data/db");
        let static_path = Path::new("/var/tmp/md0_static_files");
        /* 
        let db_env = open_db_read_only(
            db_path,
            DatabaseArguments::new(ClientVersion::default())
        ).unwrap();

        let static_file_provider = StaticFileProvider::read_only(db_path.join("hello"), true).unwrap();
        */
        let db_env = open_db_read_only(
            db_path, 
            DatabaseArguments::new(ClientVersion::default())
        ).unwrap();

        let db_env = DatabaseEnv::open(
            db_path,
            DatabaseEnvKind::RO,
            DatabaseArguments::new(ClientVersion::default())
        ).unwrap();
        let db_ev = Arc::new(db_env);

        let static_file_provider = StaticFileProvider::read_only(static_path, true).unwrap();
        static_file_provider.watch_directory();

        //let spec = Arc::new()




        let chain_spec = Arc::new(BASE_MAINNET.inner.clone());
        //let spec = Arc::new(ChainSpecBuilder::mainnet().chain(Chain::base_mainnet()).build());
        let database: ProviderFactory<NodeTypesWithDBAdapter<EthereumNode, _>> = 
            ProviderFactory::new(db_ev.clone(), chain_spec.clone(), static_file_provider);
        let lock = Lock::default();

        let _guard = lock.lock().unwrap();
        let provider  = database.latest().unwrap();

        Self {
            provider
        }





    /* 


        println!("blah");
        //let chainspec = ChainSpecBuilder::
        let spec = ChainSpecBuilder::mainnet().build();
        let factory =
        ProviderFactory::<NodeTypesWithDBAdapter<EthereumNode, _>>::new_with_database_path(
            db_path,
            spec.into(),
            Default::default(),
            StaticFileProvider::read_only(static_path, false).unwrap(),
        ).unwrap();
        println!("blaasdfh");
        //let db_provider = factory.provider().unwrap();
        let lock = Lock::default();
        let _guard = lock.lock().unwrap();
        println!("blahr2");
        let provider = factory.latest().unwrap();
        println!("blahrasdf2");

        Self {provider}
        */
    }

    fn request<F, R, E>(&self, f: F) -> Result<R, E>
    where
        F: FnOnce(&StateProviderBox) -> Result<R, E>,
    {
            let result = f(&self.provider);
            result
    }
}

impl Database for RethDB {
    type Error = anyhow::Error;

    fn basic(&mut self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        Self::basic_ref(self, address)
    }

    fn code_by_hash(&mut self, _code_hash: B256) -> Result<Bytecode, Self::Error> {
        panic!("This should not be called, as the code is already loaded");
    }

    fn storage(&mut self, address: Address, index: U256) -> Result<U256, Self::Error> {
        Self::storage_ref(self, address, index)
    }

    fn block_hash(&mut self, number: u64) -> Result<B256, Self::Error> {
        Self::block_hash_ref(self, number)
    }
}

impl DatabaseRef for RethDB {
    type Error = anyhow::Error;

    fn basic_ref(&self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        let account = self
            .request(|provider| provider.basic_account(address))?
            .unwrap_or_default();
        let code = self
            .request(|provider| provider.account_code(address))?
            .unwrap_or_default();

        let code = interpreter::analysis::to_analysed(Bytecode::new_raw(code.original_bytes()));

        Ok(Some(AccountInfo::new(
            account.balance,
            account.nonce,
            code.hash_slow(),
            code,
        )))
    }

    fn code_by_hash_ref(&self, _code_hash: B256) -> Result<Bytecode, Self::Error> {
        panic!("This should not be called, as the code is already loaded");
    }

    fn storage_ref(&self, address: Address, index: U256) -> Result<U256, Self::Error> {
        let value = self.request(|provider| provider.storage(address, StorageKey::from(index)))?;

        Ok(value.unwrap_or_default())
    }

    fn block_hash_ref(&self, number: u64) -> Result<B256, Self::Error> {
        let blockhash = self.request(|provider| provider.block_hash(number))?;

        if let Some(hash) = blockhash {
            Ok(B256::new(hash.0))
        } else {
            Ok(KECCAK_EMPTY)
        }
    }
}

