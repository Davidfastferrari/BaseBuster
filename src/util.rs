use alloy::primitives::address;
use alloy::primitives::Address;
use anyhow::Result;
use pool_sync::filter::fetch_top_volume_tokens;
use pool_sync::{Chain, Pool, PoolInfo};
use serde::{Deserialize, Serialize};
use std::fs::{create_dir_all, File};
use std::io::{BufReader, BufWriter};
use std::path::Path;

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
        address!("8ab4b525bfd7787fa3a9bd30598acf0b748c52a4"),
        address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"),
        address!("dac17f958d2ee523a2206206994597c13d831ec7"),
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
