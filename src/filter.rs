use crate::gen::ERC20Token::approveCall;
use crate::gen::{V2Aerodrome, V2Swap, V3Swap, V3SwapDeadline, V3SwapDeadlineTick};
use crate::AMOUNT;
use alloy::primitives::{address, Address, U160, U256};
use alloy::sol_types::{SolCall, SolValue};
use anyhow::Result;
use lazy_static::lazy_static;
use log::debug;
use log::trace;
use node_db::{InsertionType, NodeDB};
use pool_sync::{Chain, Pool, PoolInfo, PoolType};
use reqwest::header::{HeaderMap, HeaderValue};
use revm::primitives::keccak256;
use revm::primitives::Bytes;
use revm::wiring::default::TransactTo;
use revm::wiring::result::ExecutionResult;
use revm::wiring::EthereumWiring;
use revm::Evm;
use serde::{Deserialize, Serialize};
use std::fs::{create_dir_all, File};
use std::io::{BufReader, BufWriter};
use std::path::Path;
use std::str::FromStr;

// Blacklisted tokens we dont want to consider
lazy_static! {
    static ref BLACKLIST: Vec<Address> = vec![address!("be5614875952b1683cb0a2c20e6509be46d353a4")];
    static ref WETH_ADDRESS: Address = address!("4200000000000000000000000000000000000006");
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

// enum for swap dispatch
#[derive(Copy, Clone)]
enum SwapType {
    V2Basic,        // standard univ2 swap
    V2Aerodrome,    // aerodrome swap
    V3Basic,        // univ3 swap w/o deadline
    V3Deadline,     // univ3 swap w/ deadline
    V3DeadlineTick, // Slipstream v3 deadline and tick
}

// Given a set of pools, filter them down to a proper working set
pub async fn filter_pools(pools: Vec<Pool>, num_results: usize, chain: Chain) -> Vec<Pool> {
    debug!("Initial pool count before filter: {}", pools.len());

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
            top_volume_tokens.contains(&token0)
                && top_volume_tokens.contains(&token1)
                && !BLACKLIST.contains(&token0)
                && !BLACKLIST.contains(&token1)
        })
        .collect();

    debug!("Pool count after token match filter: {}", pools.len());

    // simulate swap on every pool that we have, this will filter out pools that have a pair we
    // want but dont have any liq to swap with
    let pools = filter_by_swap(pools).await;
    debug!("Pool count after swap filter: {}", pools.len());
    pools
}

// ---------------------------------------------------
// Helper functions to get all data and filter the pools
// ---------------------------------------------------

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

// Go through the pools and try to perform a swap on it. This is to test liquidity depth as we
// dont want to include paths that dont have enough liq for a swap
async fn filter_by_swap(pools: Vec<Pool>) -> Vec<Pool> {
    // pools that pass through swap filter
    let mut filtered_pools: Vec<Pool> = vec![];

    // state
    let account = address!("0000000000000000000000000000000000000001");
    let lots_of_tokens = U256::from(1e70);

    // construct the db
    let database_path = std::env::var("DB_PATH").unwrap();
    let mut nodedb = NodeDB::new(database_path).unwrap();

    // go through all the pools and try a swap on each one
    for pool in pools {
        // get the router address
        let (router_address, swap_type) = match pool.pool_type() {
            PoolType::UniswapV2 => (
                address!("4752ba5dbc23f44d87826276bf6fd6b1c372ad24"),
                SwapType::V2Basic,
            ),
            PoolType::SushiSwapV2 => (
                address!("6BDED42c6DA8FBf0d2bA55B2fa120C5e0c8D7891"),
                SwapType::V2Basic,
            ),
            PoolType::PancakeSwapV2 => (
                address!("8cFe327CEc66d1C090Dd72bd0FF11d690C33a2Eb"),
                SwapType::V2Basic,
            ),
            PoolType::BaseSwapV2 => (
                address!("327Df1E6de05895d2ab08513aaDD9313Fe505d86"),
                SwapType::V2Basic,
            ),
            PoolType::SwapBasedV2 => (
                address!("aaa3b1F1bd7BCc97fD1917c18ADE665C5D31F066"),
                SwapType::V2Basic,
            ),
            PoolType::DackieSwapV2 => (
                address!("Ca4EAa32E7081b0c4Ba47e2bDF9B7163907Fe56f"),
                SwapType::V2Basic,
            ),
            PoolType::AlienBaseV2 => (
                address!("8c1A3cF8f83074169FE5D7aD50B978e1cD6b37c7"),
                SwapType::V2Basic,
            ),
            PoolType::UniswapV3 => (
                address!("2626664c2603336E57B271c5C0b26F421741e481"),
                SwapType::V3Basic,
            ),
            PoolType::AlienBaseV3 => (
                address!("B20C411FC84FBB27e78608C24d0056D974ea9411"),
                SwapType::V3Basic,
            ),
            PoolType::DackieSwapV3 => (
                address!("195FBc5B8Fbd5Ac739C1BA57D4Ef6D5a704F34f7"),
                SwapType::V3Basic,
            ),
            PoolType::PancakeSwapV3 => (
                address!("678Aa4bF4E210cf2166753e054d5b7c31cc7fa86"),
                SwapType::V3Basic,
            ),
            PoolType::SushiSwapV3 => (
                address!("FB7eF66a7e61224DD6FcD0D7d9C3be5C8B049b9f"),
                SwapType::V3Deadline,
            ),
            PoolType::SwapBasedV3 => (
                address!("756C6BbDd915202adac7beBB1c6C89aC0886503f"),
                SwapType::V3Deadline,
            ),
            PoolType::BaseSwapV3 => (
                address!("1B8eea9315bE495187D873DA7773a874545D9D48"),
                SwapType::V3Deadline,
            ),
            PoolType::Aerodrome => (
                address!("cF77a3Ba9A5CA399B7c97c74d54e5b1Beb874E43"),
                SwapType::V2Aerodrome,
            ),
            PoolType::Slipstream => (
                address!("BE6D8f0d05cC4be24d5167a3eF062215bE6D18a5"),
                SwapType::V3DeadlineTick,
            ),
            _ => panic!("will not reach here"),
        };

        // Determine if this is a WETH pool and set swap direction
        let is_weth_pool =
            pool.token0_address() == *WETH_ADDRESS || pool.token1_address() == *WETH_ADDRESS;
        let zero_to_one = if is_weth_pool {
            // If WETH pool, first swap should be WETH -> Token
            pool.token0_address() == *WETH_ADDRESS
        } else {
            // For non-WETH pools, keep original direction
            true
        };

        let balance_slots = [
            keccak256((account, U256::from(0)).abi_encode()),
            keccak256((account, U256::from(1)).abi_encode()),
            keccak256((account, U256::from(2)).abi_encode()),
            keccak256((account, U256::from(3)).abi_encode()),
            keccak256((account, U256::from(4)).abi_encode()),
            keccak256((account, U256::from(5)).abi_encode()),
            //keccak256((account, U256::from(6)).abi_encode()),
            keccak256((account, U256::from(7)).abi_encode()),
            keccak256((account, U256::from(8)).abi_encode()),
            keccak256((account, U256::from(9)).abi_encode()),
        ];

        // give everyone lots of tokens, diff contracts have difference balance slots,
        // so it it just easiest to insert into all slots and have approval commit change
        // anything
        for slot in balance_slots {
            // give everyone lots of tokens
            nodedb
                .insert_account_storage(
                    pool.token0_address(),
                    slot.into(),
                    lots_of_tokens,
                    InsertionType::OnChain,
                )
                .unwrap();
            nodedb
                .insert_account_storage(
                    pool.token1_address(),
                    slot.into(),
                    lots_of_tokens,
                    InsertionType::OnChain,
                )
                .unwrap();
        }


        // construct a new evm instance
        let mut evm = Evm::<EthereumWiring<&mut NodeDB, ()>>::builder()
            .with_db(&mut nodedb)
            .with_default_ext_ctx()
            .modify_cfg_env(|env| {
                env.disable_nonce_check = true;
            })
            .modify_tx_env(|tx| {
                tx.caller = account;
                tx.value = U256::ZERO;
            })
            .build();

        // approve both the input and output token
        let approve_calldata = approveCall {
            spender: router_address,
            amount: lots_of_tokens,
        }
        .abi_encode();
        evm.tx_mut().data = approve_calldata.into();
        evm.tx_mut().transact_to = TransactTo::Call(pool.token0_address());
        evm.transact_commit().unwrap();
        evm.tx_mut().transact_to = TransactTo::Call(pool.token1_address());
        evm.transact_commit().unwrap();

        // we now have some of the input token and we have approved the router to spend it
        // try a swap to see if if it is valid
        let amt = U256::from(1e18);
        let lower_bound = amt
            .checked_mul(U256::from(90))
            .unwrap()
            .checked_div(U256::from(100))
            .unwrap();
        evm.tx_mut().transact_to = TransactTo::Call(router_address);

        // First swap (WETH -> Token for WETH pools)
        let (first_swap_calldata, vec_ret) =
            setup_router_calldata(pool.clone(), account, amt, swap_type, zero_to_one);
        evm.tx_mut().data = first_swap_calldata.into();
        let ref_tx = evm.transact().unwrap();
        let result = ref_tx.result;
        let amt = if let ExecutionResult::Success { .. } = result {
            let output = result.output().unwrap();
            decode_swap_return(output, vec_ret)
        } else {
            continue;
        };

        // Second swap (Token -> WETH for WETH pools)
        let (second_swap_calldata, vec_ret) =
            setup_router_calldata(pool.clone(), account, amt, swap_type, !zero_to_one);
        evm.tx_mut().data = second_swap_calldata.into();
        let ref_tx = evm.transact().unwrap();
        let result = ref_tx.result;
        let amt = if let ExecutionResult::Success { .. } = result {
            let output = result.output().unwrap();
            decode_swap_return(output, vec_ret)
        } else {
            println!("Failed here amt {} {:#?} {:#?}", amt, pool, result);
            continue;
        };

        // confirm that the output amount is within our reasonable error bounds
        if amt >= lower_bound {
            //println!("{:#?}", pool);
            filtered_pools.push(pool.clone());
        }
    }

    filtered_pools
}

// Swap returns are either an vec of u256, or final u256
fn decode_swap_return(output: &Bytes, vec_ret: bool) -> U256 {
    if vec_ret {
        let decoded_amount = <Vec<U256>>::abi_decode(output, false).unwrap();
        *decoded_amount.last().unwrap()
    } else {
        <U256>::abi_decode(output, false).unwrap()
    }
}

// setup the calldata for the router
fn setup_router_calldata(
    pool: Pool,
    account: Address,
    amt: U256,
    swap_type: SwapType,
    zero_to_one: bool,
) -> (Vec<u8>, bool) {
    // setup calldata based on the swap type
    let (token0, token1) = if zero_to_one {
        (pool.token0_address(), pool.token1_address())
    } else {
        (pool.token1_address(), pool.token0_address())
    };

    match swap_type {
        SwapType::V2Basic => {
            let calldata = V2Swap::swapExactTokensForTokensCall {
                amountIn: amt,
                amountOutMin: U256::ZERO,
                path: vec![token0, token1],
                to: account,
                deadline: U256::MAX,
            }
            .abi_encode();
            (calldata, true)
        }
        SwapType::V3Basic => {
            let swap_fee = pool.get_v3().unwrap().fee;
            let params = V3Swap::ExactInputSingleParams {
                tokenIn: token0,
                tokenOut: token1,
                fee: swap_fee.try_into().unwrap(),
                recipient: account,
                amountIn: amt,
                amountOutMinimum: U256::ZERO,
                sqrtPriceLimitX96: U160::ZERO,
            };
            (V3Swap::exactInputSingleCall { params }.abi_encode(), false)
        }
        SwapType::V3Deadline => {
            let swap_fee = pool.get_v3().unwrap().fee;
            let params = V3SwapDeadline::ExactInputSingleParams {
                tokenIn: token0,
                tokenOut: token1,
                fee: swap_fee.try_into().unwrap(),
                recipient: account,
                amountIn: amt,
                deadline: U256::MAX,
                amountOutMinimum: U256::ZERO,
                sqrtPriceLimitX96: U160::ZERO,
            };
            (
                V3SwapDeadline::exactInputSingleCall { params }.abi_encode(),
                false,
            )
        }
        SwapType::V2Aerodrome => {
            let is_stable = pool.get_v2().unwrap().stable.unwrap();
            let route = vec![V2Aerodrome::Route {
                from: token0,
                to: token1,
                stable: is_stable,
                factory: Address::ZERO,
            }];
            let calldata = V2Aerodrome::swapExactTokensForTokensCall {
                amountIn: amt,
                amountOutMin: U256::ZERO,
                routes: route,
                to: account,
                deadline: U256::MAX,
            }
            .abi_encode();
            (calldata, true)
        }
        SwapType::V3DeadlineTick => {
            let tick_spacing = pool.get_v3().unwrap().tick_spacing;
            let params = V3SwapDeadlineTick::ExactInputSingleParams {
                tokenIn: token0,
                tokenOut: token1,
                tickSpacing: tick_spacing.try_into().unwrap(),
                recipient: account,
                deadline: U256::MAX,
                amountIn: amt,
                amountOutMinimum: U256::ZERO,
                sqrtPriceLimitX96: U160::ZERO,
            };
            (
                V3SwapDeadlineTick::exactInputSingleCall { params }.abi_encode(),
                false,
            )
        }
    }
}
