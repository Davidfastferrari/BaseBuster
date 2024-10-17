use alloy::primitives::{address, Address};
use anyhow::Result;
use lazy_static::lazy_static;
use pool_sync::{Chain, Pool, PoolInfo};
use reqwest::header::{HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use std::fs::{create_dir_all, File};
use std::io::{BufReader, BufWriter};
use std::path::Path;
use std::str::FromStr;

// Blacklisted tokens we dont want to consider
lazy_static! {
    static ref BLACKLIST: Vec<Address> = vec![
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
}

// Serialializtion/Deserialization Structs
#[derive(Serialize, Deserialize)]
struct TopVolumeAddresses(Vec<Address>);

#[derive(Debug, Deserialize)]
struct BirdeyeResponse {
    data: ResponseData,
}

#[derive(Debug, Deserialize)]
struct ResponseData {
    tokens: Vec<Token>,
}

#[derive(Debug, Deserialize)]
struct Token {
    address: String,
}

// Given a set of pools, filter them down to a proper working set
pub async fn filter_pools(pools: Vec<Pool>, num_results: usize, chain: Chain) -> Vec<Pool> {
    // get all of the top volume tokens from birdeye, we imply volume = volatility
    let top_volume_tokens = get_top_volume_tokens(chain, num_results)
        .await
        .expect("Failed to get top volume tokens");

    // cross match top volume tokens to all pools, we want to only keep a pool if its pair exists
    // in the top volume tokens
    let pools: Vec<Pool> = pools
        .into_iter()
        .filter(|pool| {
            let token0 = pool.token0_address();
            let token1 = pool.token1_address();
            top_volume_tokens.contains(&token0) && top_volume_tokens.contains(&token1)
        })
        .collect();

    // simulate swap on every pool that we have, this will filter out pools that have a pair we
    // want but dont have any liq to swap with

    pools
}

// Helper functions to get all data and filter the pools

// fetch all the top volume tokens from birdeye
async fn get_top_volume_tokens(chain: Chain, num_results: usize) -> Result<Vec<Address>> {
    // if we have cached these tokens, just read them in
    let cache_file = format!("cache/top_volume_tokens_{}.json", chain);
    if Path::new(&cache_file).exists() {
        return read_addresses_from_file(&cache_file);
    }

    // cache for tokens does not exist, fetch them from birdeye
    let top_volume_tokens = fetch_top_volume_tokens(num_results, chain).await;

    // write tokens to file
    create_dir_all("cache").unwrap();
    write_addresses_to_file(&top_volume_tokens, &cache_file).unwrap();

    Ok(top_volume_tokens)
}

// write addresses to file
fn write_addresses_to_file(addresses: &[Address], filename: &str) -> std::io::Result<()> {
    let file = File::create(filename)?;
    let writer = BufWriter::new(file);
    let address_set = TopVolumeAddresses(addresses.to_vec());
    serde_json::to_writer(writer, &address_set)?;
    Ok(())
}

// read addresses from file
fn read_addresses_from_file(filename: &str) -> Result<Vec<Address>> {
    let file = File::open(filename)?;
    let reader = BufReader::new(file);
    let address_set: TopVolumeAddresses = serde_json::from_reader(reader)?;
    Ok(address_set.0)
}

// fetch the top volume tokens from birdeye
async fn fetch_top_volume_tokens(num_results: usize, chain: Chain) -> Vec<Address> {
    let client = reqwest::Client::new();
    let mut headers = HeaderMap::new();
    let api_key = std::env::var("BIRDEYE_KEY").unwrap();
    headers.insert("X-API-KEY", HeaderValue::from_str(&api_key).unwrap());
    if chain == Chain::Ethereum {
        headers.insert("x-chain", HeaderValue::from_static("ethereum"));
    } else if chain == Chain::Base {
        headers.insert("x-chain", HeaderValue::from_static("base"));
    }

    let mut query_params: Vec<(usize, usize)> = Vec::new();

    if num_results < 50 {
        query_params.push((0, num_results));
    } else {
        for offset in (0..num_results).step_by(50) {
            query_params.push((offset, 50));
        }
    }

    let mut addresses: Vec<String> = Vec::new();
    for (offset, num) in query_params {
        let response = client
            .get("https://public-api.birdeye.so/defi/tokenlist")
            .headers(headers.clone())
            .query(&[
                ("sort_by", "v24hUSD"),
                ("sort_type", "desc"),
                ("offset", &offset.to_string()),
                ("limit", &num.to_string()),
            ])
            .send()
            .await
            .unwrap();
        if response.status().is_success() {
            let birdeye_response: BirdeyeResponse = response.json().await.unwrap();
            let results: Vec<String> = birdeye_response
                .data
                .tokens
                .into_iter()
                .map(|token| token.address)
                .collect();
            addresses.extend(results);
        }
    }
    addresses
        .into_iter()
        .map(|addr| Address::from_str(&addr).unwrap())
        .collect()
}
