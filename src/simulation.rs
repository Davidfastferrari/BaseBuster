use alloy::network::TransactionBuilder;
use alloy::primitives::{address, Address, U256};
use alloy::providers::{Provider, ProviderBuilder};
use alloy::rpc::types::TransactionRequest;
use alloy::sol;
use alloy::node_bindings::Anvil;
use alloy::sol_types::{SolCall, SolValue};
use anyhow::Result;
use revm::primitives::Bytes;
use std::sync::Arc;

sol! {
    #[sol(rpc)]
    contract OfficialQuoter {
        struct QuoteExactInputSingleParams {
            address tokenIn;
            address tokenOut;
            uint256 amountIn;
            uint24 fee;
            uint160 sqrtPriceLimitX96;
        }

        function quoteExactInputSingle(QuoteExactInputSingleParams memory params)
        public
        override
        returns (
            uint256 amountOut,
            uint160 sqrtPriceX96After,
            uint32 initializedTicksCrossed,
            uint256 gasEstimate
        );
    }
}

pub fn decode_quote_calldata(calldata: Bytes) -> Result<u128> {
    let (amount_out, _, _, _) = <(u128, u128, u32, u128)>::abi_decode(&calldata, false)?;
    Ok(amount_out)
}

pub fn quote_calldata(
    token_in: Address,
    token_out: Address,
    amount_in: U256,
    fee: u32,
) -> OfficialQuoter::QuoteExactInputSingleParams {
    let zero_for_one = token_in < token_out;

    let sqrt_price_limit_x96: U256 = if zero_for_one {
        "4295128749".parse().unwrap()
    } else {
        "1461446703485210103287273052203988822378723970341"
            .parse()
            .unwrap()
    };

    let params = OfficialQuoter::QuoteExactInputSingleParams {
        tokenIn: token_in,
        tokenOut: token_out,
        amountIn: amount_in,
        fee,
        sqrtPriceLimitX96: sqrt_price_limit_x96,
    };
    params
}

pub fn build_tx(to: Address, from: Address, calldata: Bytes, base_fee: u128) -> TransactionRequest {
    TransactionRequest::default()
        .to(to)
        .from(from)
        .with_input(calldata)
        .nonce(0)
        .gas_limit(1000000)
        .max_fee_per_gas(base_fee)
        .max_priority_fee_per_gas(0)
        .build_unsigned()
        .unwrap()
        .into()
}

pub fn volumes(from: U256, to: U256, count: usize) -> Vec<U256> {
    let start = U256::from(0);
    let mut volumes = Vec::new();
    let distance = to - from;
    let step = distance / U256::from(count);

    for i in 1..(count + 1) {
        let current = start + step * U256::from(i);
        volumes.push(current);
    }

    volumes.reverse();
    volumes
}


pub async fn simulate_quote() {
    let url = std::env::var("HTTP").unwrap();
    let provider = ProviderBuilder::new().on_http(url.parse().unwrap());
    let base_fee = provider.get_gas_price().await.unwrap();
    let fork_block = provider.get_block_number().await.unwrap();
    let anvil = Anvil::new()
        .fork(url)
        .fork_block_number(fork_block)
        .try_spawn()
        .unwrap();
    let anvil_provider = Arc::new(ProviderBuilder::new().on_http(anvil.endpoint_url()));
    /* 
    let anvil = Anvil::new()
        .fork(url.parse().unwrap())
        .fork_block_numer(fork_block)
        .block_time(1_u64)
        .spawn();
    */


    let weth = address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");
    let usdc = address!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48");
    let amount_in = U256::from(1e17 as u64);
    let amounts_in = volumes(U256::from(0), amount_in, 100);

    let official_quoter = OfficialQuoter::new(
        address!("61fFE014bA17989E743c5F6cB21bF9697530B21e"),
        anvil_provider.clone(),
    );
    let start = std::time::Instant::now();
    let params = quote_calldata(weth, usdc, amounts_in[0], 3000);
    let OfficialQuoter::quoteExactInputSingleReturn {
        amountOut,
        sqrtPriceX96After,
        initializedTicksCrossed,
        gasEstimate,
    } = official_quoter
        .quoteExactInputSingle(params)
        .call()
        .await
        .unwrap();
    println!("First call took {:?}", start.elapsed());


    let start2 = std::time::Instant::now();
    for(index, volume) in amounts_in.into_iter().enumerate() {
        let params = quote_calldata(weth, usdc, volume, 3000);
        let OfficialQuoter::quoteExactInputSingleReturn {
            amountOut,
            sqrtPriceX96After,
            initializedTicksCrossed,
            gasEstimate,
        } = official_quoter
            .quoteExactInputSingle(params)
            .call()
            .await
            .unwrap();
        //println!("{} WETH -> {} USDC", volume, amountOut);
    }
    let end = start2.elapsed(); 
    println!("Remaining Took: {:?}", end);
}
