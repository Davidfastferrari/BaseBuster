use alloy::primitives::{address, Address};
use anyhow::Result;
use pool_sync::fetch_top_volume_tokens;
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
pub async fn filter_pools(pools: Vec<Pool>, num_results: usize, chain: Chain) -> Vec<Pool> {
    // get all the top volume tokens
    let mut top_volume_tokens = get_top_volume_tokens(chain, num_results).await.unwrap();
    /* 
    let blacklist = vec![
        address!("60a3E35Cc302bFA44Cb288Bc5a4F316Fdb1adb42"),
        address!("04D5ddf5f3a8939889F11E97f8c4BB48317F1938"),
        address!("fde4C96c8593536E31F229EA8f37b2ADa2699bb2"),
        address!("d9aAEc86B65D86f6A7B5B1b0c42FFA531710b6CA"),
        address!("B79DD08EA68A908A97220C76d19A6aA9cBDE4376"),
        address!("2Ae3F1Ec7F1F5012CFEab0185bfc7aa3cf0DEc22"),
        address!("2416092f143378750bb29b79eD961ab195CcEea5"),
        address!("50c5725949A6F0c72E6C4a641F24049A917DB0Cb"),
        address!("833589fCD6eDb6E08f4c7C32D4f71b54bdA02913"),
    ];
    top_volume_tokens.retain(|token| !blacklist.contains(token));
    */

    pools
        .into_iter()
        .filter(|pool| {
            let token0 = pool.token0_address();
            let token1 = pool.token1_address();
            top_volume_tokens.contains(&token0) && top_volume_tokens.contains(&token1)
        })
        .collect()
}

pub fn get_routers() -> Vec<Address> {
    vec![
        address!("4752ba5DBc23f44D87826276BF6Fd6b1C372aD24"), // UNISWAP_V2_ROUTER
        address!("6BDED42c6DA8FBf0d2bA55B2fa120C5e0c8D7891"), // SUSHISWAP_V2_ROUTER
        address!("8cFe327CEc66d1C090Dd72bd0FF11d690C33a2Eb"), // PANCAKESWAP_V2_ROUTER
        address!("327Df1E6de05895d2ab08513aaDD9313Fe505d86"), // BASESWAP_V2_ROUTER
        address!("2626664c2603336E57B271c5C0b26F421741e481"), // UNISWAP_V3_ROUTER
        address!("678Aa4bF4E210cf2166753e054d5b7c31cc7fa86"), // PANCAKESWAP_V3_ROUTER
        address!("FB7eF66a7e61224DD6FcD0D7d9C3be5C8B049b9f"), // SUSHISWAP_V3_ROUTER
        address!("1B8eea9315bE495187D873DA7773a874545D9D48"), // BASESWAP_V3_ROUTER
        address!("BE6D8f0d05cC4be24d5167a3eF062215bE6D18a5"), // SLIPSTREAM_ROUTER
        address!("cF77a3Ba9A5CA399B7c97c74d54e5b1Beb874E43"), // AERODOME_ROUTER
        address!("e20fCBdBfFC4Dd138cE8b2E6FBb6CB49777ad64D"), // AAVE_ADDRESSES_PROVIDER
        address!("BA12222222228d8Ba445958a75a0704d566BF2C8"), // BALANCER_VAULT
    ]
}