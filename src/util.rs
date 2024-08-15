use alloy::network::EthereumWallet;
use alloy::node_bindings::{Anvil, AnvilInstance};
use alloy::primitives::{address, Address};
use alloy::providers::{Provider, ProviderBuilder};
use alloy::signers::local::PrivateKeySigner;
use anyhow::Result;
use log::info;
use pool_sync::filter::fetch_top_volume_tokens;
use pool_sync::{Chain, Pool, PoolInfo};
use serde::{Deserialize, Serialize};
use std::fs::{create_dir_all, File};
use std::io::{BufReader, BufWriter};
use std::path::Path;
use std::sync::Arc;

use crate::FlashSwap;
#[derive(Serialize, Deserialize)]
struct TopVolumeAddresses(Vec<Address>);

// write addresses to file
pub fn write_addresses_to_file(addresses: &Vec<Address>, filename: &str) -> std::io::Result<()> {
    let file = File::create(filename)?;
    let writer = BufWriter::new(file);
    let address_set = TopVolumeAddresses(addresses.clone());
    serde_json::to_writer(writer, &address_set)?;
    Ok(())
}

// read addresses from file
pub fn read_addresses_from_file(filename: &str) -> Result<Vec<Address>> {
    let file = File::open(filename)?;
    let reader = BufReader::new(file);
    let address_set: TopVolumeAddresses = serde_json::from_reader(reader)?;
    Ok(address_set.0)
}

// fetch all the top volume tokens
pub async fn get_top_volume_tokens(chain: Chain, num_results: usize) -> Result<Vec<Address>> {
    // path
    let cache_dir = "cache";
    let cache_file = format!("{}/top_volume_tokens_{}.json", cache_dir, chain);

    // if the path exists, read from the file
    if Path::new(&cache_file).exists() {
        return read_addresses_from_file(&cache_file);
    }
    // file does not exists, fetch them
    let top_volume_tokens = fetch_top_volume_tokens(num_results, chain).await;

    // create dir and write it to file
    create_dir_all(cache_dir).unwrap();
    write_addresses_to_file(&top_volume_tokens, &cache_file).unwrap();

    Ok(top_volume_tokens)
}

// based on the top volume tokens, load in all of the working pools
pub async fn get_working_pools(pools: Vec<Pool>, num_results: usize, chain: Chain) -> Vec<Pool> {
    // get all the top volume tokens
    let mut top_volume_tokens = get_top_volume_tokens(chain, num_results).await.unwrap();
    let blacklist = vec![
        address!("4752ba5dbc23f44d87826276bf6fd6b1c372ad24"),
        address!("236aa50979d5f3de3bd1eeb40e81137f22ab794b"),
        address!("8b03d30b88e86fc5f447069c79ec56b8e7d87ab6"),
        address!("B79DD08EA68A908A97220C76d19A6aA9cBDE4376"),
        address!("d9aAEc86B65D86f6A7B5B1b0c42FFA531710b6CA"),
        address!("fde4C96c8593536E31F229EA8f37b2ADa2699bb2"),
        address!("940181a94A35A4569E4529A3CDfB74e38FD98631"),
        address!("04D5ddf5f3a8939889F11E97f8c4BB48317F1938"),
        address!("c1CBa3fCea344f92D9239c08C0568f6F2F0ee452"),
        address!("50c5725949A6F0c72E6C4a641F24049A917DB0Cb"),
    ];
    top_volume_tokens.retain(|token| !blacklist.contains(token));

    pools
        .into_iter()
        .filter(|pool| {
            let token0 = pool.token0_address();
            let token1 = pool.token1_address();
            top_volume_tokens.contains(&token0) && top_volume_tokens.contains(&token1)
        })
        .collect()
}

// deploy the flash swap contract
pub async fn deploy_flash_swap() -> (AnvilInstance, Address) {
    let http_provider =
        Arc::new(ProviderBuilder::new().on_http(std::env::var("FULL").unwrap().parse().unwrap()));
    let fork_block = http_provider.get_block_number().await.unwrap();
    let anvil = Anvil::new()
        .fork(std::env::var("FULL").unwrap())
        .port(9100_u16)
        .fork_block_number(fork_block)
        .try_spawn()
        .unwrap();

    info!("Anvil started on {}", anvil.endpoint_url());
    let signer: PrivateKeySigner = anvil.keys()[0].clone().into();
    let wallet = EthereumWallet::from(signer);
    let anvil_signer = Arc::new(
        ProviderBuilder::new()
            .with_recommended_fillers()
            .network::<alloy::network::AnyNetwork>()
            .wallet(wallet)
            .on_http(anvil.endpoint_url()),
    );

    let flash_contract = FlashSwap::deploy(anvil_signer.clone()).await.unwrap();
    let flash_address = flash_contract.address();
    info!("FlashSwap deployed at: {}", flash_address);
    (anvil, *flash_address)
}
