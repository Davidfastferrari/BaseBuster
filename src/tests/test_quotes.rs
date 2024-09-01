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

// All offchain calculation tests
#[cfg(test)]
mod offchain_calculations {
    use super::*;

    // UNISWAPV2
    #[tokio::test]
    pub async fn test_uniswapv2_out() {
        let swap_step = SwapStep {
            pool_address: address!("88a43bbdf9d098eec7bceda4e2494615dfd9bb9c"),
            token_in: address!("4200000000000000000000000000000000000006"),
            token_out: address!("833589fcd6edb6e08f4c7c32d4f71b54bda02913"),
            protocol: PoolType::UniswapV2,
            fee: 0,
        };
        let amount_in = U256::from(1e16);
        let offchain_amount_out = offchain_quote(&swap_step, amount_in).await;
        let onchain_amount_out = onchain_quote(&swap_step, PoolType::UniswapV2, amount_in).await;
        assert_eq!(offchain_amount_out, onchain_amount_out);
    }

    // UNISWAPV3
    #[tokio::test]
    pub async fn test_uniswapv3_out() {
        let swap_step = SwapStep {
            pool_address: address!("a2d4a8e00daad32acace1a0dd0905f6aaf57e84e"),
            token_in: address!("4200000000000000000000000000000000000006"),
            token_out: address!("6985884c4392d348587b19cb9eaaf157f13271cd"),
            protocol: PoolType::UniswapV3,
            fee: 3000,
        };
        let amount_in = U256::from(1e16);
        let offchain_amount_out = offchain_quote(&swap_step, amount_in).await;
        let onchain_amount_out = onchain_quote(&swap_step, PoolType::UniswapV3, amount_in).await;
        assert_eq!(offchain_amount_out, onchain_amount_out);
    }

    // SUSHISWAPV2
    #[tokio::test]
    pub async fn test_sushiswapv2_out() {
        let swap_step = SwapStep {
            pool_address: address!("98433581b5420bc67fc5fd2b5f9dd3e7ca43140b"),
            token_in: address!("47b464edb8dc9bc67b5cd4c9310bb87b773845bd"),
            token_out: address!("4200000000000000000000000000000000000006"),
            protocol: PoolType::SushiSwapV2,
            fee: 0,
        };

        let amount_in = U256::from(1e16);
        let offchain_amount_out = offchain_quote(&swap_step, amount_in).await;
        let onchain_amount_out = onchain_quote(&swap_step, PoolType::SushiSwapV2, amount_in).await;
        assert_eq!(offchain_amount_out, onchain_amount_out);
    }

    // SUSHISWAPV3
    #[tokio::test]
    pub async fn test_sushiswapv3_out() {
        let swap_step =     SwapStep {
            pool_address: address!("a73f10b99551f6e08609ccdec5ff66d51e4e3700"),
            token_in: address!("532f27101965dd16442e59d40670faf5ebb142e4"),
            token_out: address!("833589fcd6edb6e08f4c7c32d4f71b54bda02913"),
            protocol: PoolType::SushiSwapV3,
            fee: 10000,
        };
        let amount_in = U256::from(1e16);
        let offchain_amount_out = offchain_quote(&swap_step, amount_in).await;
        let onchain_amount_out = onchain_quote(&swap_step, PoolType::SushiSwapV3, amount_in).await;
        assert_eq!(offchain_amount_out, onchain_amount_out);
    }

    // PANCAKESWAPV2
    #[tokio::test]
    pub async fn test_pancakeswapv2_out() {
        let swap_step = SwapStep {
            pool_address: address!("60824b0543410d824291c29be32284456fcf1f8e"),
            token_in: address!("2ae3f1ec7f1f5012cfeab0185bfc7aa3cf0dec22"),
            token_out: address!("833589fcd6edb6e08f4c7c32d4f71b54bda02913"),
            protocol: PoolType::PancakeSwapV2,
            fee: 0,
        };
        let amount_in = U256::from(1e16);
        let offchain_amount_out = offchain_quote(&swap_step, amount_in).await;
        let onchain_amount_out = onchain_quote(&swap_step, PoolType::PancakeSwapV2, amount_in).await;
        assert_eq!(offchain_amount_out, onchain_amount_out);
    }


    // PANCAKESWAPV3
    #[tokio::test]
    pub async fn test_pancakeswapv3_out() {
        let swap_step = SwapStep {
                pool_address: address!("3c288a41c135fb0bae3f95b6a37b5e3e89f3fd95"),
                token_in: address!("4200000000000000000000000000000000000006"),
                token_out: address!("b1a03eda10342529bbf8eb700a06c60441fef25d"),
                protocol: PoolType::PancakeSwapV3,
                fee: 500,
            };
        let amount_in = U256::from(1e16);
        let offchain_amount_out = offchain_quote(&swap_step, amount_in).await;
        let onchain_amount_out = onchain_quote(&swap_step, PoolType::PancakeSwapV3, amount_in).await;
        assert_eq!(offchain_amount_out, onchain_amount_out);
    }

    // AERODROME
    #[tokio::test]
    pub async fn test_aerodrome_out() {
        let swap_step =     SwapStep {
            pool_address: address!("acb7907c232907934b2578315dfcfa1ba60e87af"),
            token_in: address!("9beec80e62aa257ced8b0edd8692f79ee8783777"),
            token_out: address!("4200000000000000000000000000000000000006"),
            protocol: PoolType::Aerodrome,
            fee: 0,
        };
        let amount_in = U256::from(1e16);
        let offchain_amount_out = offchain_quote(&swap_step, amount_in).await;
        let onchain_amount_out = onchain_quote(&swap_step, PoolType::Aerodrome, amount_in).await;
        assert_eq!(offchain_amount_out, onchain_amount_out);
    }

    // SLIPSTREAM
    #[tokio::test]
    pub async fn test_slipstream_out() {
        todo!()
    }

    // BASESWAPV2
    #[tokio::test]
    pub async fn test_baseswapv2_out() {
        let swap_step = SwapStep {
            pool_address: address!("1be25ca7954b8ce47978851a0689312518d85f0c"),
            token_in: address!("2ae3f1ec7f1f5012cfeab0185bfc7aa3cf0dec22"),
            token_out: address!("833589fcd6edb6e08f4c7c32d4f71b54bda02913"),
            protocol: PoolType::BaseSwapV2,
            fee: 0,
        };

        let amount_in = U256::from(1e16);
        let offchain_amount_out = offchain_quote(&swap_step, amount_in).await;
        let onchain_amount_out = onchain_quote(&swap_step, PoolType::BaseSwapV2, amount_in).await;
        assert_eq!(offchain_amount_out, onchain_amount_out);
    }

    // BASESWAP V3
    #[tokio::test]
    pub async fn test_baseswapv3_out() {
        let swap_step = SwapStep {
            pool_address: address!("74cb6260be6f31965c239df6d6ef2ac2b5d4f020"),
            token_in: address!("4200000000000000000000000000000000000006"),
            token_out: address!("833589fcd6edb6e08f4c7c32d4f71b54bda02913"),
            protocol: PoolType::BaseSwapV3,
            fee: 0,
        };
        let amount_in = U256::from(1e16);
        let offchain_amount_out = offchain_quote(&swap_step, amount_in).await;
        let onchain_amount_out = onchain_quote(&swap_step, PoolType::BaseSwapV3, amount_in).await;
        assert_eq!(offchain_amount_out, onchain_amount_out);
    }

    // ALIENBASE
    #[tokio::test]
    pub async fn test_alienbase_out() {
        let swap_step = SwapStep {
            pool_address: address!("74cb6260be6f31965c239df6d6ef2ac2b5d4f020"),
            token_in: address!("4200000000000000000000000000000000000006"),
            token_out: address!("833589fcd6edb6e08f4c7c32d4f71b54bda02913"),
            protocol: PoolType::AlienBase,
            fee: 0,
        };
        let amount_in = U256::from(1e16);
        let offchain_amount_out = offchain_quote(&swap_step, amount_in).await;
        let onchain_amount_out = onchain_quote(&swap_step, PoolType::AlienBase, amount_in).await;
        assert_eq!(offchain_amount_out, onchain_amount_out);
    }

    // MAVERICKV1
    #[tokio::test(flavor = "multi_thread")]
    pub async fn test_maverickv1_out() {
        todo!()
    }

    // MAVERICKV2
    #[tokio::test(flavor = "multi_thread")]
    pub async fn test_maverickv2_out() {
        let swap_step = SwapStep {
            pool_address: address!("5b6a0771c752e35b2ca2aff4f22a66b1598a2bc5"),
            token_in: address!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"),
            token_out: address!("dac17f958d2ee523a2206206994597c13d831ec7"),
            protocol: PoolType::MaverickV2,
            fee: 0,
        };
        todo!()
    }


    // BALANCER
    #[tokio::test(flavor = "multi_thread")]
    pub async fn test_balancer_out() {
        let swap_step = SwapStep {
            pool_address: address!("98b76fb35387142f97d601a297276bb152ae8ab0"),
            token_in: address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
            token_out: address!("faba6f8e4a5e8ab82f62fe7c39859fa577269be3"),
            protocol: PoolType::BalancerV2,
            fee: 0,
        };
        todo!()
    }


    // CURVETWO
    #[tokio::test(flavor = "multi_thread")]
    pub async fn test_curve_two_out() {
        dotenv::dotenv().ok();
        let swap_step = SwapStep {
            pool_address: address!("004C167d27ADa24305b76D80762997Fa6EB8d9B2"),
            token_in: address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
            token_out: address!("97efFB790f2fbB701D88f89DB4521348A2B77be8"),
            protocol: PoolType::CurveTwoCrypto,
            fee: 0,
        };
        todo!()
    }

    // CURVETRI
    #[tokio::test(flavor = "multi_thread")]
    pub async fn test_curvetri_out() {
        dotenv::dotenv().ok();
        let swap_step = SwapStep {
            pool_address: address!("7F86Bf177Dd4F3494b841a37e810A34dD56c829B"),
            token_in: address!("A0b869b91c6218b36c1d19D4a2e9Eb0cE3606eB48"),
            token_out: address!("2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599"),
            protocol: PoolType::CurveTriCrypto,
            fee: 0,
        };
        todo!()
    }

    // DACKIESWAPV2
    pub async fn test_dackiswapv2_out() {
        todo!()
    }

    // DACKIESWAPV3
    pub async fn test_dackiswapv3_out() {
        todo!()
    }

    // SWAPBASEDV2
    pub async fn test_swapbasedv2_out() {
        todo!()
    }

    // SWAPBASEDV3
    pub async fn test_swapbasedv3_out() {
        todo!()
    }
}



// use the onchain qouters to get the amount out for a single swap
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
    } else if pool_type.is_curve_two() {
        todo!()
    } else if pool_type.is_curve_tri() {
        todo!()
    } else if pool_type.is_maverick() {
        todo!()
    } else {
        panic!("have not done this yet")
    }
    
}


pub async fn offchain_quote(
    swap_step: &SwapStep,
    amount_in: U256,
) -> U256 {
    todo!()
}


// Get the onchain quote for a v2 pool
pub async fn onchain_v2(swap_step: &SwapStep, pool_type: PoolType, amount_in: U256) -> U256 {
    sol!(
        #[sol(rpc)]
        contract V2Router {
            function getAmountsOut(uint amountIn, address[] calldata path) external view returns (uint[] memory amounts);
        }
    );
    let provider = ProviderBuilder::new().on_http(std::env::var("FULL").unwrap().parse().unwrap());


    let address = match pool_type {
        PoolType::UniswapV2 => address!("4752ba5DBc23f44D87826276BF6Fd6b1C372aD24"),
        PoolType::SushiSwapV2 => address!("6BDED42c6DA8FBf0d2bA55B2fa120C5e0c8D7891"),
        PoolType::PancakeSwapV2 => address!("8cFe327CEc66d1C090Dd72bd0FF11d690C33a2Eb"),
        PoolType::BaseSwapV2 => address!("327Df1E6de05895d2ab08513aaDD9313Fe505d86"),
        PoolType::SwapBasedV2 => address!("327Df1E6de05895d2ab08513aaDD9313Fe505d86"), // this address is wrong
        PoolType::DackieSwapV2 => address!("327Df1E6de05895d2ab08513aaDD9313Fe505d86"), // this address is wrong
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
    sol!(
        #[sol(rpc)]
        contract V3Quoter {
            struct QuoteExactInputSingleParams {
                address tokenIn;
                address tokenOut;
                uint256 amountIn;
                uint24 fee;
                uint160 sqrtPriceLimitX96;
            }
            function quoteExactInputSingle(QuoteExactInputSingleParams memory params)
            external
            returns (
                uint256 amountOut,
                uint160 sqrtPriceX96After,
                uint32 initializedTicksCrossed,
                uint256 gasEstimate
            );

        }
    );

    let provider = ProviderBuilder::new().on_http(std::env::var("FULL").unwrap().parse().unwrap());

    let address = match pool_type {
        PoolType::UniswapV3 => address!("3d4e44Eb1374240CE5F1B871ab261CD16335B76a"),
        PoolType::PancakeSwapV3 => address!("B048Bbc1Ee6b733FFfCFb9e9CeF7375518e25997"),
        PoolType::SushiSwapV3 => address!("b1E835Dc2785b52265711e17fCCb0fd018226a6e"),
        PoolType::Slipstream => address!("254cF9E1E6e233aa1AC962CB9B05b2cfeAaE15b0"),
        PoolType::AlienBase => address!("8c1A3cF8f83074169FE5D7aD50B978e1cD6b37c7"),
        PoolType::BaseSwapV3 => address!("327Df1E6de05895d2ab08513aaDD9313Fe505d86"), // this address is wrong
        PoolType::SwapBasedV3 => address!("327Df1E6de05895d2ab08513aaDD9313Fe505d86"), // this address is wrong
        PoolType::DackieSwapV3 => address!("327Df1E6de05895d2ab08513aaDD9313Fe505d86"), // this address is wrong
        _ => panic!("Invalid pool type"),
    };

    let contract = V3Quoter::new(address, provider.clone());

    let params = V3Quoter::QuoteExactInputSingleParams {
        tokenIn: swap_step.token_in,
        tokenOut: swap_step.token_out,
        fee: swap_step.fee,
        amountIn: amount_in,
        sqrtPriceLimitX96: U256::ZERO,
    };

    let V3Quoter::quoteExactInputSingleReturn { amountOut , ..} = contract
        .quoteExactInputSingle(params)
        .call()
        .await
        .unwrap();
    return amountOut;
}


// Get the onchain quote for a balancer pool
pub async fn onchain_balancer(swap_step: &SwapStep, amount_in: U256) -> U256 {
    sol!(
        #[sol(rpc)]
        contract BalancerV2Vault {
            enum SwapKind { GIVEN_IN, GIVEN_OUT }

            struct BatchSwapStep {
                bytes32 poolId;
                uint256 assetInIndex;
                uint256 assetOutIndex;
                uint256 amount;
                bytes userData;
            }

            struct FundManagement {
                address sender;
                bool fromInternalBalance;
                address payable recipient;
                bool toInternalBalance;
            }

            function queryBatchSwap(
                SwapKind kind,
                BatchSwapStep[] memory swaps,
                address[] memory assets,
                FundManagement memory funds
            ) external returns (int256[] memory);
        }
    );

    sol!(
        #[sol(rpc)]
        contract BalancerPool {
            function getPoolId() external view returns (bytes32);
        }
    );

    // get the pool id
    let provider = ProviderBuilder::new().on_http(std::env::var("FULL").unwrap().parse().unwrap());
    let contract = BalancerPool::new(swap_step.pool_address, provider);
    let BalancerPool::getPoolIdReturn { _0: pool_id } = contract.getPoolId().call().await.unwrap();

    // make the vault
    let vault_address = address!("BA12222222228d8Ba445958a75a0704d566BF2C8");
    let contract = BalancerV2Vault::new(vault_address, provider);


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

    let transaction = contract.queryBatchSwap(
        BalancerV2Vault::SwapKind::GIVEN_IN,
        vec![single_swap],
        vec![swap_step.token_in, swap_step.token_out],
        fund_management,
    ).into_transaction_request();
    let provider =
        ProviderBuilder::new().on_http(std::env::var("FULL").unwrap().parse().unwrap());
    
    let trace = provider.debug_trace_call(transaction, alloy::eips::BlockNumberOrTag::Latest, get_tracing_options().clone()).await.unwrap();
    println!("Trace: {:#?}", trace);

    // Process the trace to extract the amount out
    // This part depends on how you want to interpret the trace
    // For now, we'll return a placeholder value
    U256::from(0)
}


// Curve two pool
pub async fn onchain_curve_two(swap_step: &SwapStep, pool_type: PoolType, amount_in: U256) -> U256 {
    sol!(
        #[sol(rpc)]
        contract CurveTwoCryptoPool {
            function get_dy(int128 i, int128 j, uint256 dx) external view returns (uint256);
        }
    );

    let provider = ProviderBuilder::new().on_http(std::env::var("FULL").unwrap().parse().unwrap());
    j



    todo!()
}


pub async fn onchain_curve_tri(swap_step: &SwapStep, pool_type: PoolType, amount_in: U256) -> U256 {
    todo!()
}

pub async fn onchain_maverick(swap_step: &SwapStep, pool_type: PoolType, amount_in: U256) -> U256 {
    todo!()
}

