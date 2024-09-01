use alloy::network::Ethereum;
use alloy::network::EthereumWallet;
use alloy::node_bindings::Anvil;
use alloy::node_bindings::AnvilInstance;
use alloy::primitives::U256;
use alloy::primitives::{address, Address};
use alloy::providers::ext::DebugApi;
use alloy::providers::{Provider, ProviderBuilder, RootProvider, WsConnect};
use alloy::rpc::types::trace::geth::GethDebugBuiltInTracerType::CallTracer;
use alloy::rpc::types::trace::geth::GethDebugTracingOptions;
use alloy::rpc::types::trace::geth::{
    CallConfig, CallFrame, GethDebugTracerConfig, GethDebugTracerType,
    GethDebugTracingCallOptions, GethDefaultTracingOptions, GethTrace,
};
use alloy::signers::local::PrivateKeySigner;
use alloy::sol;
use alloy::transports::http::{Client, Http};
use alloy::primitives::FixedBytes;
use gweiyser::addresses::amms;
use gweiyser::protocols::uniswap::v2::UniswapV2Pool;
use gweiyser::protocols::uniswap::v3::UniswapV3Pool;
use gweiyser::{Chain, Gweiyser};
use pool_sync::*;
use revm::interpreter::instructions::contract;
use sha2::digest::consts::U25;
use std::sync::Arc;
use tokio::sync::broadcast;

use crate::calculation::Calculator;
use crate::events::Event;
use crate::graph::SwapStep;
use crate::pool_manager;
use crate::pool_manager::PoolManager;
use crate::util::get_working_pools;
use crate::FlashSwap;

sol!(
    #[derive(Debug)]
    #[sol(rpc)]
    FlashQuoter,
    "src/abi/FlashQuoter.json"
);





#[cfg(test)]
pub mod flash_swap {
    use super::*;
    #[tokio::test]
    pub async fn debug_swap() {
        dotenv::dotenv().ok();
        let swaps = vec![        
            SwapStep {
                pool_address: address!("f282e7c46be1a3758357a5961cf02e1f46a78b75"),
                token_in: address!("4200000000000000000000000000000000000006"),
                token_out: address!("2075f6e2147d4ac26036c9b4084f8e28b324397d"),
                protocol: PoolType::UniswapV2,
                fee: 0,
            },
            SwapStep {
                pool_address: address!("f609cdba05f08e850676f7434db0d9468b3701bd"),
                token_in: address!("2075f6e2147d4ac26036c9b4084f8e28b324397d"),
                token_out: address!("6921b130d297cc43754afba22e5eac0fbf8db75b"),
                protocol: PoolType::UniswapV3,
                fee: 10000,
            },
            SwapStep {
                pool_address: address!("088c39ee29fc30df8adc394e9f7dea33e3a26507"),
                token_in: address!("6921b130d297cc43754afba22e5eac0fbf8db75b"),
                token_out: address!("4200000000000000000000000000000000000006"),
                protocol: PoolType::Slipstream,
                fee: 8000,
            },
        ];

        let converted_path: Vec<FlashSwap::SwapStep> = swaps
            .clone()
            .iter()
            .map(|step| FlashSwap::SwapStep {
                poolAddress: step.pool_address,
                tokenIn: step.token_in,
                tokenOut: step.token_out,
                protocol: step.as_u8(),
                fee: step.fee,
            })
            .collect();

        //calculate_full_quote(swaps).await;
        let profit = simulate_profit(converted_path).await;
        println!("Profit: {:?}", profit);
    }

}




#[cfg(test)]
mod test_path_quotes{
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn quote() {
        dotenv::dotenv().ok();
        let swaps = vec![
            SwapStep {
                pool_address: address!("4114fd8554e63a9e0f09ca2480977883fea06430"),
                token_in: address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
                token_out: address!("dac17f958d2ee523a2206206994597c13d831ec7"),
                protocol: PoolType::BalancerV2,
                fee: 0,
            },
            SwapStep {
                pool_address: address!("e618785149a1ee7d3042e304e2f899f7a4616b7d"),
                token_in: address!("dac17f958d2ee523a2206206994597c13d831ec7"),
                token_out: address!("34950ff2b487d9e5282c5ab342d08a2f712eb79f"),
                protocol: PoolType::UniswapV2,
                fee: 0,
            },
            SwapStep {
                pool_address: address!("1b7143e445b4d1424fa24f0c3ba0c5778da43c5b"),
                token_in: address!("34950ff2b487d9e5282c5ab342d08a2f712eb79f"),
                token_out: address!("1f9840a85d5af5bf1d1762f925bdaddc4201f984"),
                protocol: PoolType::UniswapV2,
                fee: 0,
            },
            SwapStep {
                pool_address: address!("d3d2e2692501a5c9ca623199d38826e513033a17"),
                token_in: address!("1f9840a85d5af5bf1d1762f925bdaddc4201f984"),
                token_out: address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
                protocol: PoolType::UniswapV2,
                fee: 0,
            },
        ];


        let amount = U256::from(1e16);
        let simulated_quote = simulate_full_quote(swaps.clone(), amount).await;
        let calculated_quote = calculate_full_quote(swaps, amount).await;
        //println!("Profit: {:?}", simulated_quote);
        println!("Calculated profit: {:?}", calculated_quote);

    }


}







// HELPER FUNCTIONS FOR THE TESTS
// ------------------------------------------------------

// Use the quoter contract to get the amount out from a swap path
pub async fn simulate_full_quote(swap_steps: Vec<SwapStep>, amount: U256) -> U256 {
    // deploy the quoter
    let url = std::env::var("FULL").unwrap();
    let anvil = Anvil::new()
        .fork(url)
        .port(9100_u16)
        .try_spawn()
        .unwrap();

    let signer: PrivateKeySigner = anvil.keys()[0].clone().into();
    let wallet = EthereumWallet::from(signer);
    let anvil_signer = Arc::new(
        ProviderBuilder::new()
            .with_recommended_fillers()
            //.network::<alloy::network::AnyNetwork>()
            .wallet(wallet)
            .on_http(anvil.endpoint_url()),
    );
    let flash_quoter = FlashQuoter::deploy(anvil_signer.clone()).await.unwrap();
    let gweiyser = Gweiyser::new(anvil_signer.clone(), Chain::Ethereum);
    let weth = gweiyser.token(address!("4200000000000000000000000000000000000006")).await;
    let weth = gweiyser.token(address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2")).await;
    weth.deposit(amount).await; // deposit into signers account, account[0] here
    println!("got here");
    weth.transfer_from(anvil.addresses()[0], *flash_quoter.address(), amount).await;
    println!("got hereasdf");
    weth.approve(*flash_quoter.address(), amount).await;
    println!("got hereasdfasdf");

    let converted_path = swappath_to_flashquote(swap_steps.clone()).await;

    let provider = ProviderBuilder::new().on_http("http://localhost:9100".parse().unwrap());
    let flash_quoter = FlashQuoter::new(*flash_quoter.address(), provider.clone());
    let FlashQuoter::executeArbitrageReturn { _0: profit } = flash_quoter
        .executeArbitrage(converted_path, amount)
        .from(anvil.addresses()[0])
        .call()
        .await
        .unwrap();
    profit
}


// use out offchain calculations to get the amount out from a swap path
pub async fn calculate_full_quote(steps: Vec<SwapStep>, amount: U256) -> U256 {
    // get all of the pools in the swap path and put them into the pool manager
    let pools = steps.iter().map(|step| step.pool_address).collect(); 
    let (pool_manager, mut reserve_receiver) = pool_manager_with_pools(pools).await;
    let calculator = Calculator::new().await;

    // wait for a new update so we are working wtih fresh set
    let mut amount_in = amount;
    if let Ok(_) = reserve_receiver.recv().await {
        for step in steps {
            println!("Amount in: {}", amount_in);
            amount_in = calculator.get_amount_out(amount_in, &pool_manager, &step);

            println!("Amount out: {}", amount_in);
        }
    }
    amount_in
}


// use the offchain calculations to get the amount out for a single swap
pub async fn calculate_single_quote(swap_step: SwapStep, amount_in: U256) -> U256 {
    let (pool_manager, mut reserve_receiver) =
        pool_manager_with_pools(vec![swap_step.pool_address]).await;

    let calculator = Calculator::new().await;
    if let Ok(_) = reserve_receiver.recv().await {
        let output = calculator.get_amount_out(amount_in, &pool_manager, &swap_step);
        return output;
    }
    U256::ZERO
}



// simulated profit based off of the call to the contract
async fn simulate_profit(steps: Vec<FlashSwap::SwapStep>) -> Option<U256> {
    let url = std::env::var("FULL").unwrap();
    let provider = ProviderBuilder::new().on_http(url.parse().unwrap());

    let anvil = Anvil::new()
        .fork(url)
        .port(9100_u16)
        .try_spawn()
        .unwrap();
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
    let options = get_tracing_options();

    let provider =
        Arc::new(ProviderBuilder::new().on_http("http://localhost:9100".parse().unwrap()));
    let contract = FlashSwap::new(*flash_address, provider.clone());

    //println!("{:?}", FlashSwap::executeArbitrageCall::SELECTOR);
    let tx = contract
        .executeArbitrage(steps, U256::from(2e15))
        .from(anvil.addresses()[0])
        .into_transaction_request();
    let output = provider
        .debug_trace_call(tx, alloy::eips::BlockNumberOrTag::Latest, options.clone())
        .await
        .unwrap();
    println!("Output: {:#?}", output);
    match output {
        GethTrace::CallTracer(call_trace) => {
            //let output = process_output(&call_trace);

            let res = extract_final_balance(&call_trace);
            return res;
        }
        _ => return None,
    }
}

// Get a v3 pool from gweiyser
pub async fn get_v3_pool(
    pool_address: &Address,
) -> UniswapV3Pool<RootProvider<Http<Client>>, Http<Client>, Ethereum> {
    let provider = Arc::new(
        ProviderBuilder::new()
            .network::<Ethereum>()
            .on_http(std::env::var("FULL").unwrap().parse().unwrap()),
    );
    let gweiyser = Gweiyser::new(provider.clone(), Chain::Base);
    gweiyser.uniswap_v3_pool(*pool_address).await
}

// Get a v3 pool fro gweiyser
pub async fn get_v2_pool(
    pool_address: Address,
) -> UniswapV2Pool<RootProvider<Http<Client>>, Http<Client>, Ethereum> {
    let provider = Arc::new(
        ProviderBuilder::new()
            .network::<Ethereum>()
            .on_http(std::env::var("FULL").unwrap().parse().unwrap()),
    );
    let gweiyser = Gweiyser::new(provider.clone(), Chain::Base);
    gweiyser.uniswap_v2_pool(pool_address).await
}





async fn get_balancer_pool_id(pool_address: Address) -> FixedBytes<32> {
    sol!(
        #[sol(rpc)]
        contract BalancerPool {
            function getPoolId() external view returns (bytes32);
        }
    );

    let provider = ProviderBuilder::new().on_http(std::env::var("FULL").unwrap().parse().unwrap());
    let contract = BalancerPool::new(pool_address, provider);

    let BalancerPool::getPoolIdReturn { _0: pool_id } = contract.getPoolId().call().await.unwrap();
    pool_id
}
