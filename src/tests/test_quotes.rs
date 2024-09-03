use alloy::network::Ethereum;
use alloy::network::EthereumWallet;
use alloy::node_bindings::Anvil;
use alloy::primitives::Signed;
use alloy::node_bindings::AnvilInstance;
use alloy::sol_types::{SolValue, SolCall};
use revm::primitives::ExecutionResult;
use crate::db::RethDB;
use alloy::primitives::U256;
use alloy::primitives::{address, Address};
use alloy::providers::ext::DebugApi;
use alloy::providers::{Provider, ProviderBuilder, RootProvider, WsConnect};
use alloy::rpc::types::trace::geth::GethDebugBuiltInTracerType::CallTracer;
use revm::db::CacheDB;
use alloy::eips::BlockId;
use alloy::eips::BlockNumberOrTag;
use alloy::primitives::U160;
use revm::primitives::keccak256;
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
use revm::primitives::TransactTo;
use revm::Evm;
use sha2::digest::consts::U25;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::broadcast;

use super::test_gen::*;
use crate::calculation::Calculator;
use crate::events::Event;
use crate::graph::SwapStep;
use crate::pool_manager;
use crate::pool_manager::PoolManager;
use crate::util::get_working_pools;
use crate::FlashSwap;
use super::test_utils::*;

// All offchain calculation tests
#[cfg(test)]
mod offchain_calculations {
    use futures::StreamExt;

    use super::*;

    // UNISWAPV2
    #[tokio::test(flavor = "multi_thread")]
    pub async fn test_uniswapv2_out() {
        dotenv::dotenv().ok();

        let ws = WsConnect::new(std::env::var("WS").unwrap());
        let ws = Arc::new(ProviderBuilder::new().on_ws(ws).await.unwrap());
        let sub = ws.subscribe_blocks().await.unwrap();
        let mut stream = sub.into_stream().take(10);

        let amount_in = U256::from(1e16);
        let (pool_manager , _) = pool_manager_with_type(PoolType::UniswapV2).await;

        let swap_step = SwapStep {
            pool_address: address!("B4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc"),
            token_in: address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
            token_out: address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"),
            protocol: PoolType::UniswapV2,
            fee: 0,
        };

        while let Some(_) = stream.next().await {
            let offchain_amount_out = offchain_quote(&swap_step, PoolType::UniswapV2, amount_in, &pool_manager).await;
            let onchain_amount_out = onchain_quote(&swap_step, PoolType::UniswapV2, amount_in).await;
            println!("offchain: {:?}, onchain: {:?}", offchain_amount_out, onchain_amount_out);
            assert_eq!(offchain_amount_out, onchain_amount_out);
        }

    }

    // UNISWAPV3
    #[tokio::test(flavor = "multi_thread")]
    pub async fn test_uniswapv3_out() {
        dotenv::dotenv().ok();

        let ws = WsConnect::new(std::env::var("WS").unwrap());
        let ws = Arc::new(ProviderBuilder::new().on_ws(ws).await.unwrap());
        let sub = ws.subscribe_blocks().await.unwrap();
        let mut stream = sub.into_stream().take(10);

        let amount_in = U256::from(1e16);
        let (pool_manager , _) = pool_manager_with_type(PoolType::UniswapV3).await;

        let swap_step = SwapStep {
            pool_address: address!("88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640"),
            token_in: address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
            token_out: address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"),
            protocol: PoolType::UniswapV3,
            fee: 500,
        };

        while let Some(_) = stream.next().await {
            let offchain_amount_out = offchain_quote(&swap_step, PoolType::UniswapV3, amount_in, &pool_manager).await;
            let onchain_amount_out = onchain_quote(&swap_step, PoolType::UniswapV3, amount_in).await;
            println!("offchain: {:?}, onchain: {:?}", offchain_amount_out, onchain_amount_out);
            assert_eq!(offchain_amount_out, onchain_amount_out);
        }
    }

    // SUSHISWAPV2
    #[tokio::test(flavor = "multi_thread")]
    pub async fn test_sushiswapv2_out() {
        dotenv::dotenv().ok();

        let ws = WsConnect::new(std::env::var("WS").unwrap());
        let ws = Arc::new(ProviderBuilder::new().on_ws(ws).await.unwrap());
        let sub = ws.subscribe_blocks().await.unwrap();
        let mut stream = sub.into_stream().take(10);

        let amount_in = U256::from(1e16);
        let (pool_manager , _) = pool_manager_with_type(PoolType::SushiSwapV2).await;

        let swap_step = SwapStep {
            pool_address: address!("06da0fd433c1a5d7a4faa01111c044910a184553"),
            token_in: address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
            token_out: address!("dac17f958d2ee523a2206206994597c13d831ec7"),
            protocol: PoolType::SushiSwapV2,
            fee: 0,
        };

        while let Some(_) = stream.next().await {
            let offchain_amount_out = offchain_quote(&swap_step, PoolType::SushiSwapV2, amount_in, &pool_manager).await;
            let onchain_amount_out = onchain_quote(&swap_step, PoolType::SushiSwapV2, amount_in).await;
            println!("offchain: {:?}, onchain: {:?}", offchain_amount_out, onchain_amount_out);
            assert_eq!(offchain_amount_out, onchain_amount_out);
        }
    }

    // SUSHISWAPV3
    #[tokio::test(flavor = "multi_thread")]
    pub async fn test_sushiswapv3_out() {
        dotenv::dotenv().ok();

        let ws = WsConnect::new(std::env::var("WS").unwrap());
        let ws = Arc::new(ProviderBuilder::new().on_ws(ws).await.unwrap());
        let sub = ws.subscribe_blocks().await.unwrap();
        let mut stream = sub.into_stream().take(10);

        let amount_in = U256::from(1e16);
        let (pool_manager , _) = pool_manager_with_type(PoolType::SushiSwapV3).await;

        let swap_step =     SwapStep {
            pool_address: address!("35644Fb61aFBc458bf92B15AdD6ABc1996Be5014"),
            token_in: address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
            token_out: address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"),
            protocol: PoolType::SushiSwapV3,
            fee: 500,
        };

        while let Some(_) = stream.next().await {
            let offchain_amount_out = offchain_quote(&swap_step, PoolType::SushiSwapV3, amount_in, &pool_manager).await;
            let onchain_amount_out = onchain_quote(&swap_step, PoolType::SushiSwapV3, amount_in).await;
            println!("offchain: {:?}, onchain: {:?}", offchain_amount_out, onchain_amount_out);
            assert_eq!(offchain_amount_out, onchain_amount_out);
        }
    }

    // PANCAKESWAPV2
    #[tokio::test(flavor = "multi_thread")]
    pub async fn test_pancakeswapv2_out() {
        dotenv::dotenv().ok();

        let ws = WsConnect::new(std::env::var("WS").unwrap());
        let ws = Arc::new(ProviderBuilder::new().on_ws(ws).await.unwrap());
        let sub = ws.subscribe_blocks().await.unwrap();
        let mut stream = sub.into_stream().take(10);

        let amount_in = U256::from(1e16);
        let (pool_manager , _) = pool_manager_with_type(PoolType::PancakeSwapV2).await;

        let swap_step = SwapStep {
            pool_address: address!("2E8135bE71230c6B1B4045696d41C09Db0414226"),
            token_in: address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
            token_out: address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"),
            protocol: PoolType::PancakeSwapV2,
            fee: 0,
        };

        while let Some(_) = stream.next().await {
            let offchain_amount_out = offchain_quote(&swap_step, PoolType::PancakeSwapV2, amount_in, &pool_manager).await;
            let onchain_amount_out = onchain_quote(&swap_step, PoolType::PancakeSwapV2, amount_in).await;
            println!("offchain: {:?}, onchain: {:?}", offchain_amount_out, onchain_amount_out);
            assert_eq!(offchain_amount_out, onchain_amount_out);
        }
    }


    // PANCAKESWAPV3
    #[tokio::test(flavor = "multi_thread")]
    pub async fn test_pancakeswapv3_out() {
        dotenv::dotenv().ok();

        let ws = WsConnect::new(std::env::var("WS").unwrap());
        let ws = Arc::new(ProviderBuilder::new().on_ws(ws).await.unwrap());
        let sub = ws.subscribe_blocks().await.unwrap();
        let mut stream = sub.into_stream().take(10);

        let amount_in = U256::from(1e16);
        let (pool_manager , _) = pool_manager_with_type(PoolType::PancakeSwapV3).await;

        let swap_step = SwapStep {
            pool_address: address!("1ac1A8FEaAEa1900C4166dEeed0C11cC10669D36"),
            token_in: address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
            token_out: address!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"),
            protocol: PoolType::PancakeSwapV3,
            fee: 500,
        };

        
        while let Some(_) = stream.next().await {
            let offchain_amount_out = offchain_quote(&swap_step, PoolType::PancakeSwapV3, amount_in, &pool_manager).await;
            let onchain_amount_out = onchain_quote(&swap_step, PoolType::PancakeSwapV3, amount_in).await;
            println!("offchain: {:?}, onchain: {:?}", offchain_amount_out, onchain_amount_out);
            assert_eq!(offchain_amount_out, onchain_amount_out);
        }
    }

    // CURVETWO
    #[tokio::test(flavor = "multi_thread")]
    pub async fn test_curve_two_out() {
        dotenv::dotenv().ok();

        let ws = WsConnect::new(std::env::var("WS").unwrap());
        let ws = Arc::new(ProviderBuilder::new().on_ws(ws).await.unwrap());
        let sub = ws.subscribe_blocks().await.unwrap();
        let mut stream = sub.into_stream().take(10);

        let amount_in = U256::from(1e16);
        let (pool_manager , _) = pool_manager_with_type(PoolType::CurveTwoCrypto).await;

        let swap_step = SwapStep {
            pool_address: address!("ca546aE6c3B2BB9Fba2b6e5EeB0881097CecE5B0"),
            token_in: address!("f939E0A03FB07F59A73314E73794Be0E57ac1b4E"),
            token_out: address!("1cfa5641c01406aB8AC350dEd7d735ec41298372"),
            protocol: PoolType::CurveTwoCrypto,
            fee: 0,
        };

        while let Some(_) = stream.next().await {
            let offchain_amount_out = offchain_quote(&swap_step, PoolType::CurveTwoCrypto, amount_in, &pool_manager).await;
            let onchain_amount_out = onchain_quote(&swap_step, PoolType::CurveTwoCrypto, amount_in).await;
            println!("offchain: {:?}, onchain: {:?}", offchain_amount_out, onchain_amount_out);
            assert_eq!(offchain_amount_out, onchain_amount_out);
        }
    }

    // CURVETRI
    #[tokio::test(flavor = "multi_thread")]
    pub async fn test_curve_tri_out() {
        dotenv::dotenv().ok();

        let ws = WsConnect::new(std::env::var("WS").unwrap());
        let ws = Arc::new(ProviderBuilder::new().on_ws(ws).await.unwrap());
        let sub = ws.subscribe_blocks().await.unwrap();
        let mut stream = sub.into_stream().take(10);

        let amount_in = U256::from(1e10);
        let (pool_manager , _) = pool_manager_with_type(PoolType::CurveTriCrypto).await;

        let swap_step = SwapStep {
            pool_address: address!("7F86Bf177Dd4F3494b841a37e810A34dD56c829B"),
            token_in: address!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"),
            token_out: address!("2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599"),
            protocol: PoolType::CurveTriCrypto,
            fee: 0,
        };

        while let Some(_) = stream.next().await {
            let offchain_amount_out = offchain_quote(&swap_step, PoolType::CurveTriCrypto, amount_in, &pool_manager).await;
            let onchain_amount_out = onchain_quote(&swap_step, PoolType::CurveTriCrypto, amount_in).await;
            println!("offchain: {:?}, onchain: {:?}", offchain_amount_out, onchain_amount_out);
            assert_eq!(offchain_amount_out, onchain_amount_out);
        }
    }

    // MAVERICKV2
    #[tokio::test(flavor = "multi_thread")]
    pub async fn test_maverickv2_out() {
        dotenv::dotenv().ok();

        let ws = WsConnect::new(std::env::var("WS").unwrap());
        let ws = Arc::new(ProviderBuilder::new().on_ws(ws).await.unwrap());
        let sub = ws.subscribe_blocks().await.unwrap();
        let mut stream = sub.into_stream().take(10);

        let amount_in = U256::from(1e17);

        let (pool_manager , _) = pool_manager_with_type(PoolType::MaverickV2).await;
        let swap_step = SwapStep {
            pool_address: address!("9Cc6044F0FC2e3896A37509dB7837Efa01F6413D"),
            token_in: address!("7448c7456a97769F6cD04F1E83A4a23cCdC46aBD"),
            token_out: address!("C54Ff26fd5564Ff46b14d9825A2259a0d53Bf7d9"),
            protocol: PoolType::MaverickV2,
            fee: 0,
        };

        while let Some(_) = stream.next().await {
            let offchain_amount_out = offchain_quote(&swap_step, PoolType::MaverickV2, amount_in, &pool_manager).await;
            let onchain_amount_out = onchain_quote(&swap_step, PoolType::MaverickV2, amount_in).await;
            println!("offchain: {:?}, onchain: {:?}", offchain_amount_out, onchain_amount_out);
            assert_eq!(offchain_amount_out, onchain_amount_out);
        }
    }

    // BALANCER
    #[tokio::test(flavor = "multi_thread")]
    pub async fn test_balancer_out() {
        dotenv::dotenv().ok();

        let ws = WsConnect::new(std::env::var("WS").unwrap());
        let ws = Arc::new(ProviderBuilder::new().on_ws(ws).await.unwrap());
        let sub = ws.subscribe_blocks().await.unwrap();
        let mut stream = sub.into_stream();

        let amount_in = U256::from(1e17);

        let (pool_manager , _) = pool_manager_with_type(PoolType::BalancerV2).await;

        let swap_step = SwapStep {
            pool_address: address!("3de27EFa2F1AA663Ae5D458857e731c129069F29"),
            token_in: address!("7f39C581F595B53c5cb19bD0b3f8dA6c935E2Ca0"),
            token_out: address!("7Fc66500c84A76Ad7e9c93437bFc5Ac33E2DDaE9"),
            protocol: PoolType::BalancerV2,
            fee: 0,
        };

        while let Some(_) = stream.next().await {
            let start = Instant::now();
            let offchain_amount_out = offchain_quote(&swap_step, PoolType::BalancerV2, amount_in, &pool_manager).await;
            let onchain_amount_out = onchain_quote(&swap_step, PoolType::BalancerV2, amount_in).await;
            let end = Instant::now();
            println!("onchain out took {:?}", end.duration_since(start));
            println!("offchain: {:?}, onchain: {:?}", offchain_amount_out, onchain_amount_out);
            assert_eq!(offchain_amount_out, onchain_amount_out);
        }
    }

    /* 
    // AERODROME
    #[tokio::test(flavor = "multi_thread")]
    pub async fn test_aerodrome_out() {
        let swap_step =     SwapStep {
            pool_address: address!("acb7907c232907934b2578315dfcfa1ba60e87af"),
            token_in: address!("9beec80e62aa257ced8b0edd8692f79ee8783777"),
            token_out: address!("4200000000000000000000000000000000000006"),
            protocol: PoolType::Aerodrome,
            fee: 0,
        };
        let amount_in = U256::from(1e16);
        let offchain_amount_out = offchain_quote(&swap_step, PoolType::Aerodrome, amount_in).await;
        let onchain_amount_out = onchain_quote(&swap_step, PoolType::Aerodrome, amount_in).await;
        assert_eq!(offchain_amount_out, onchain_amount_out);
    }

    // SLIPSTREAM
    #[tokio::test(flavor = "multi_thread")]
    pub async fn test_slipstream_out() {
        todo!()
    }

    // BASESWAPV2
    #[tokio::test(flavor = "multi_thread")]
    pub async fn test_baseswapv2_out() {
        let swap_step = SwapStep {
            pool_address: address!("1be25ca7954b8ce47978851a0689312518d85f0c"),
            token_in: address!("2ae3f1ec7f1f5012cfeab0185bfc7aa3cf0dec22"),
            token_out: address!("833589fcd6edb6e08f4c7c32d4f71b54bda02913"),
            protocol: PoolType::BaseSwapV2,
            fee: 0,
        };

        let amount_in = U256::from(1e16);
        let offchain_amount_out = offchain_quote(&swap_step, PoolType::BaseSwapV2, amount_in).await;
        let onchain_amount_out = onchain_quote(&swap_step, PoolType::BaseSwapV2, amount_in).await;
        assert_eq!(offchain_amount_out, onchain_amount_out);
    }

    // BASESWAP V3
    #[tokio::test(flavor = "multi_thread")]
    pub async fn test_baseswapv3_out() {
        let swap_step = SwapStep {
            pool_address: address!("74cb6260be6f31965c239df6d6ef2ac2b5d4f020"),
            token_in: address!("4200000000000000000000000000000000000006"),
            token_out: address!("833589fcd6edb6e08f4c7c32d4f71b54bda02913"),
            protocol: PoolType::BaseSwapV3,
            fee: 0,
        };
        let amount_in = U256::from(1e16);
        let offchain_amount_out = offchain_quote(&swap_step, PoolType::BaseSwapV3, amount_in).await;
        let onchain_amount_out = onchain_quote(&swap_step, PoolType::BaseSwapV3, amount_in).await;
        assert_eq!(offchain_amount_out, onchain_amount_out);
    }

    // ALIENBASE
    #[tokio::test(flavor = "multi_thread")]
    pub async fn test_alienbasev2_out() {
        let swap_step = SwapStep {
            pool_address: address!("74cb6260be6f31965c239df6d6ef2ac2b5d4f020"),
            token_in: address!("4200000000000000000000000000000000000006"),
            token_out: address!("833589fcd6edb6e08f4c7c32d4f71b54bda02913"),
            protocol: PoolType::AlienBaseV2,
            fee: 0,
        };
        let amount_in = U256::from(1e16);
        let offchain_amount_out = offchain_quote(&swap_step, PoolType::AlienBaseV2, amount_in).await;
        let onchain_amount_out = onchain_quote(&swap_step, PoolType::AlienBaseV2, amount_in).await;
        assert_eq!(offchain_amount_out, onchain_amount_out);
    }

    // ALIENBASEV3
    #[tokio::test(flavor = "multi_thread")]
    pub async fn test_alienbasev3_out() {
        let swap_step = SwapStep {
            pool_address: address!("74cb6260be6f31965c239df6d6ef2ac2b5d4f020"),
            token_in: address!("4200000000000000000000000000000000000006"),
            token_out: address!("833589fcd6edb6e08f4c7c32d4f71b54bda02913"),
            protocol: PoolType::AlienBaseV3,
            fee: 0,
        };
        let amount_in = U256::from(1e16);
        let offchain_amount_out = offchain_quote(&swap_step, PoolType::AlienBaseV3, amount_in).await;
        let onchain_amount_out = onchain_quote(&swap_step, PoolType::AlienBaseV3, amount_in).await;
        assert_eq!(offchain_amount_out, onchain_amount_out);
    }

    // MAVERICKV1
    #[tokio::test(flavor = "multi_thread")]
    pub async fn test_maverickv1_out() {
        todo!()
    }






    // DACKIESWAPV2
    #[tokio::test(flavor = "multi_thread")]
    pub async fn test_dackiswapv2_out() {
        todo!()
    }

    // DACKIESWAPV3
    #[tokio::test(flavor = "multi_thread")]
    pub async fn test_dackiswapv3_out() {
        todo!()
    }

    // SWAPBASEDV2
    #[tokio::test(flavor = "multi_thread")]
    pub async fn test_swapbasedv2_out() {
        todo!()
    }

    // SWAPBASEDV3
    #[tokio::test(flavor = "multi_thread")]
    pub async fn test_swapbasedv3_out() {
        todo!()
    }
    */
}



// use the onchain quoters to get the amount out for a single swap
pub async fn onchain_quote(
    swap_step: &SwapStep,
    pool_type: PoolType,
    amount_in: U256,
) -> U256 {
    dotenv::dotenv().ok();

    if pool_type.is_v2() {
        onchain_v2(swap_step, pool_type, amount_in).await
    } else if pool_type.is_v3() {
        onchain_v3(swap_step, pool_type, amount_in).await
    } else if pool_type.is_balancer() {
        onchain_balancer(swap_step, amount_in).await
    } else if pool_type.is_curve_two() || pool_type.is_curve_tri() {
        onchain_curve(swap_step, pool_type, amount_in).await
    } else if pool_type.is_maverick() {
        onchain_maverick(swap_step, pool_type, amount_in).await
    } else {
        panic!("have not done this yet")
    }
    
}

// uses the offchain calculator to get the amount out for a single swap
pub async fn offchain_quote(
    swap_step: &SwapStep,
    pool_type: PoolType,
    amount_in: U256,
    pool_manager: &PoolManager,
) -> U256 {
    dotenv::dotenv().ok();
    let calculator = Calculator::new().await;
    let start = Instant::now();
    let amt = calculator.get_amount_out(
        amount_in,
        &pool_manager,
        swap_step
    );
    let end = Instant::now();
    println!("Calculator out took {:?}", end.duration_since(start));
    amt


}


// ONCHAIN QUOTERSK
// --------------------------

// Get the onchain quote for a v2 pool
pub async fn onchain_v2(swap_step: &SwapStep, pool_type: PoolType, amount_in: U256) -> U256 {
    let provider = ProviderBuilder::new().on_http(std::env::var("FULL").unwrap().parse().unwrap());

    let address = match pool_type {
        PoolType::UniswapV2 => address!("7a250d5630B4cF539739dF2C5dAcb4c659F2488D"),
        PoolType::SushiSwapV2 => address!("d9e1cE17f2641f24aE83637ab66a2cca9C378B9F"),
        PoolType::PancakeSwapV2 => address!("EfF92A263d31888d860bD50809A8D171709b7b1c"),
        PoolType::BaseSwapV2 => address!("327Df1E6de05895d2ab08513aaDD9313Fe505d86"),
        PoolType::SwapBasedV2 => address!("aaa3b1F1bd7BCc97fD1917c18ADE665C5D31F066"), 
        PoolType::DackieSwapV2 => address!("Ca4EAa32E7081b0c4Ba47e2bDF9B7163907Fe56f"), 
        PoolType::AlienBaseV2 => address!("8c1A3cF8f83074169FE5D7aD50B978e1cD6b37c7"),
        _ => panic!("will not reach here"),
    };

    let contract = V2Router::new(address, provider);
    let V2Router::getAmountsOutReturn { amounts } = contract
        .getAmountsOut(amount_in, vec![swap_step.token_in, swap_step.token_out])
        .call()
        .await
        .unwrap();
    return *amounts.last().unwrap();
}

// Get the onchain quote for a v3 pool
pub async fn onchain_v3(swap_step: &SwapStep, pool_type: PoolType, amount_in: U256) -> U256 {
    match pool_type {
        PoolType::UniswapV3 | PoolType::SushiSwapV3 | PoolType::PancakeSwapV3 => {
            onchain_v3_quoter(pool_type, swap_step, amount_in).await
        }
        _ => onchain_v3_router(pool_type, swap_step, amount_in)
    }
}



// V3 amount out from the quoter
pub async fn onchain_v3_quoter(pool_type: PoolType, swap_step: &SwapStep, amount_in: U256 ) -> U256 {
    let provider = ProviderBuilder::new().on_http(std::env::var("FULL").unwrap().parse().unwrap());

    let address = match pool_type {
        PoolType::UniswapV3 => address!("61fFE014bA17989E743c5F6cB21bF9697530B21e"),
        PoolType::PancakeSwapV3 => address!("B048Bbc1Ee6b733FFfCFb9e9CeF7375518e25997"),
        PoolType::SushiSwapV3 => address!("64e8802FE490fa7cc61d3463958199161Bb608A7"),
        _ => panic!("Invalid pool type"),
    };

    let contract = V3Quoter::new(address, provider.clone());

    let params = V3Quoter::QuoteExactInputSingleParams {
        tokenIn: swap_step.token_in,
        tokenOut: swap_step.token_out,
        fee: swap_step.fee.try_into().unwrap(),
        amountIn: amount_in,
        sqrtPriceLimitX96: U160::ZERO,
    };

    let V3Quoter::quoteExactInputSingleReturn { amountOut , ..} = contract
        .quoteExactInputSingle(params)
        .call()
        .await
        .unwrap();
    return amountOut;
}


// V3 amount out from the router
pub fn onchain_v3_router(pool_type: PoolType, swap: &SwapStep, amount_in: U256) -> U256 {
    // setup the db
    let data_path = "/home/docker/volumes/eth-docker_reth-el-data/_data";
    let mut db = CacheDB::new(RethDB::new(data_path, None).unwrap());

    let weth = address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2");
    let account = address!("18B06aaF27d44B756FCF16Ca20C1f183EB49111f");
    let router = address!("68b3465833fb72A70ecDF485E0e4C7bD8665Fc45");

    // give our test account some fake WETH and ETH
    let weth_balance_slot = U256::from(3);
    let one_ether = U256::from(1_000_000_000_000_000_000u128);
    let hashed_acc_balance_slot = keccak256((account, weth_balance_slot).abi_encode());
    db
        .insert_account_storage(weth, hashed_acc_balance_slot.into(), one_ether)
        .unwrap();

    let mut evm = Evm::builder().with_db(db).build();

    let calldata = ERC20::approveCall {
        spender: router,
        amount: U256::from(1e18)
    }.abi_encode();

    evm.tx_mut().caller = account;
    evm.tx_mut().transact_to = TransactTo::Call(weth);
    evm.tx_mut().data = calldata.into();
    evm.tx_mut().value = U256::ZERO;
    let ref_tx = evm.transact_commit().unwrap();

    let router = match pool_type {
        PoolType::DackieSwapV3 => address!("195FBc5B8Fbd5Ac739C1BA57D4Ef6D5a704F34f7"),
        PoolType::SwapBasedV3 => address!("756C6BbDd915202adac7beBB1c6C89aC0886503f"),
        PoolType::AlienBaseV3 => address!("B20C411FC84FBB27e78608C24d0056D974ea9411"),
        PoolType::BaseSwapV3 => address!("1B8eea9315bE495187D873DA7773a874545D9D48"),
        _ => panic!("will not reach here")
    };
    
    let calldata = match pool_type {
        PoolType::BaseSwapV3 | PoolType::SwapBasedV3 => {
            let params = RouterDeadline::ExactInputSingleParams {
                tokenIn: swap.token_in,
                tokenOut: swap.token_out,
                fee: swap.fee.try_into().unwrap(),
                recipient: account,
                amountIn: amount_in,
                deadline: U256::MAX,
                amountOutMinimum: U256::ZERO,
                sqrtPriceLimitX96: U160::ZERO,
            };
            RouterDeadline::exactInputSingleCall { params }.abi_encode()
        }
        PoolType::AlienBaseV3 | PoolType::DackieSwapV3 => {
            let params = Router::ExactInputSingleParams {
                tokenIn: swap.token_in,
                tokenOut: swap.token_out,
                fee: swap.fee.try_into().unwrap(),
                recipient: account,
                amountIn: amount_in,
                amountOutMinimum: U256::ZERO,
                sqrtPriceLimitX96: U160::ZERO,
            };
            Router::exactInputSingleCall { params }.abi_encode()
        }
        PoolType::Slipstream => {
            todo!()
        }
        _ => panic!("Will not reach here")
    };

    evm.tx_mut().transact_to = TransactTo::Call(weth);
    evm.tx_mut().data = calldata.into();
    let ref_tx = evm.transact().unwrap();
    println!("{:?}", ref_tx);
    U256::ZERO
}


pub async fn onchain_slipstream(swap_step: &SwapStep, pool_type: PoolType, amount_in: U256) -> U256 {
    //254cF9E1E6e233aa1AC962CB9B05b2cfeAaE15b0
    todo!()
}

// get amount out for curve pools
pub async fn onchain_curve(swap_step: &SwapStep, pool_type: PoolType, amount_in: U256) -> U256 {
    let provider = ProviderBuilder::new().on_http(std::env::var("FULL").unwrap().parse().unwrap());

    let contract = Curve::new(swap_step.pool_address, provider);

    let Curve::get_dyReturn { _0: amount_out } = contract.get_dy(
        U256::from(0), 
        U256::from(1), 
        amount_in
    ).call().await.unwrap();
    amount_out
}

pub async fn onchain_maverick(swap_step: &SwapStep, pool_type: PoolType, amount_in: U256) -> U256 {
    let provider = ProviderBuilder::new().on_http(std::env::var("FULL").unwrap().parse().unwrap());
    let router = address!("b40AfdB85a07f37aE217E7D6462e609900dD8D7A");
    let contract = MaverickOut::new(router, provider);

    let MaverickOut::calculateSwapReturn {amountOut, .. } = contract.calculateSwap(
        swap_step.pool_address,
        amount_in.to::<u128>(),
        true,
        false,
        i32::MAX
    ).call().await.unwrap();
    amountOut
}

// Get the onchain quote for a balancer pool
pub async fn onchain_balancer(swap_step: &SwapStep, amount_in: U256) -> U256 {
    // get the pool id
    let provider = Arc::new(ProviderBuilder::new().on_http(std::env::var("FULL").unwrap().parse().unwrap()));
    let contract = BalancerPool::new(swap_step.pool_address, provider.clone());
    let BalancerPool::getPoolIdReturn { _0: pool_id } = contract.getPoolId().call().await.unwrap();

    // make the vault
    let vault_address = address!("BA12222222228d8Ba445958a75a0704d566BF2C8");

    let single_swap = BalancerV2Vault::BatchSwapStep {
        poolId: pool_id,
        assetInIndex: U256::from(0),
        assetOutIndex: U256::from(1),
        amount: amount_in,
        userData: vec![].into(),
    };

    let fund_management = BalancerV2Vault::FundManagement {
        sender: Address::ZERO,
        fromInternalBalance: false,
        recipient: Address::ZERO,
        toInternalBalance: false,
    };

    let transaction = BalancerV2Vault::queryBatchSwapCall {
        kind: BalancerV2Vault::SwapKind::GIVEN_IN,
        swaps: vec![single_swap],
        assets: vec![swap_step.token_in, swap_step.token_out],
        funds: fund_management,
    }.abi_encode();

    let data_path = "/home/docker/volumes/eth-docker_reth-el-data/_data";
    let mut db = CacheDB::new(RethDB::new(data_path, None).unwrap());

    let start = Instant::now();
    let mut evm = Evm::builder()
        .with_db(db)
        .modify_tx_env(|tx| {
            tx.caller = address!("0000000000000000000000000000000000000001");
            tx.transact_to = TransactTo::Call(vault_address);
            tx.data = transaction.into();
            tx.value = U256::ZERO;
        }).build();

    
    let ref_tx = evm.transact().unwrap();
    let result = ref_tx.result;
    match result {
        ExecutionResult::Success {
            output: value,
            ..
        } => {
            let a = match <Vec<Signed<256, 4>>>::abi_decode(&value.data(), false) {
                Ok(a) => {
                    let output = a.get(1).unwrap();
                    let abs = output.abs();
                    let res = U256::try_from(abs).unwrap();
                    let end = Instant::now();
                    println!("Balancer V2 out took {:?}", end.duration_since(start));
                    res
                }
                Err(_) => U256::ZERO
            };
            return a;
        }
        _=> U256::ZERO
    }
    //println!("Result: {:#?}", result);


    //U256::ZERO

}