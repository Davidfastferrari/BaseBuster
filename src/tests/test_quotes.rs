use alloy::primitives::{Signed, U160};
use alloy::sol_types::{SolValue, SolCall};
use revm::primitives::{Bytecode, ExecutionResult};
use alloy::primitives::U256;
use alloy::primitives::{address, Address};
use alloy::providers::{Provider, ProviderBuilder};
use revm::db::EmptyDB;
use alloy::sol;
use pool_sync::*;
use revm::primitives::{TransactTo, AccountInfo};
use revm::Evm;
use revm::db::AlloyDB;
use std::sync::Arc;
use std::time::Instant;

use super::test_gen::*;
use crate::calculation::Calculator;
use crate::market_state::MarketState;
use crate::swap::*;
use crate::FlashSwap;
use super::test_utils::*;
use crate::swap::SwapPath;
use crate::state_db::BlockStateDB;

// All offchain calculation tests
#[cfg(test)]
mod offchain_calculations {

    use super::*;

    macro_rules! test_pool_out {
        ($test_name:ident, $pool_type:ident, $pool_address:expr, $token_in:expr, $token_out:expr, $fee:expr) => {
            #[tokio::test(flavor = "multi_thread")]
            pub async fn $test_name() {
                dotenv::dotenv().ok();

                let (market, mut address_rx) = market_with_type(PoolType::$pool_type).await;

                let swap_path = SwapPath {
                    steps: vec![SwapStep { pool_address: address!($pool_address),
                        token_in: address!($token_in),
                        token_out: address!($token_out),
                        protocol: PoolType::$pool_type,
                        fee: $fee,
                    }],
                    hash: 0
                };

                while let Some(_) = address_rx.recv().await {
                    let offchain_amount_out = offchain_quote(&swap_path, market.clone()).await;
                    let onchain_amount_out = onchain_quote(&swap_path, PoolType::$pool_type, market.clone()).await;
                    println!("offchain: {:?}, onchain: {:?}", offchain_amount_out, onchain_amount_out);
                    assert_eq!(offchain_amount_out, onchain_amount_out);
                }
            }
        };
    }


    // V2 VARIANTS
    // ---------------------------
    test_pool_out!(test_uniswapv2_out, UniswapV2, "B4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc", "C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2", "A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48", 0); // OK
    //test_pool_out!(test_uniswapv2_out, UniswapV2, "88A43bbDF9D098eEC7bCEda4e2494615dfD9bB9C", "4200000000000000000000000000000000000006", "833589fCD6eDb6E08f4c7C32D4f71b54bdA02913", 0); // OK
    test_pool_out!(test_sushiswapv2_out, SushiSwapV2, "2F8818D1B0f3e3E295440c1C0cDDf40aAA21fA87", "4200000000000000000000000000000000000006", "833589fCD6eDb6E08f4c7C32D4f71b54bdA02913", 0); // OK
    test_pool_out!(test_pancakeswapv2_out, PancakeSwapV2, "79474223AEdD0339780baCcE75aBDa0BE84dcBF9", "4200000000000000000000000000000000000006", "833589fCD6eDb6E08f4c7C32D4f71b54bdA02913", 0); // OK
    test_pool_out!(test_baseswapv2_out, BaseSwapV2, "ab067c01C7F5734da168C699Ae9d23a4512c9FdB", "4200000000000000000000000000000000000006", "833589fCD6eDb6E08f4c7C32D4f71b54bdA02913", 0); // OK
    test_pool_out!(test_swapbasedv2_out, SwapBasedV2, "aEeB835f3Aa21d19ea5E33772DaA9E64f1b6982F", "4200000000000000000000000000000000000006", "833589fCD6eDb6E08f4c7C32D4f71b54bdA02913", 0); // OK
    test_pool_out!(test_alienbasev2_out, AlienBaseV2, "B16D2257643fdBB32d12b9d73faB784eB4f1Bee4", "4200000000000000000000000000000000000006", "833589fCD6eDb6E08f4c7C32D4f71b54bdA02913", 0); // OK
    test_pool_out!(test_dackieswapv2_out, DackieSwapV2, "6bee1580471F38000951abd788A9C060A4ad3Ac3", "4200000000000000000000000000000000000006", "833589fCD6eDb6E08f4c7C32D4f71b54bdA02913", 0); // OK


    // V3 VARIANTS
    //-----------------------
    test_pool_out!(test_uniswapv3_out, UniswapV3, "d0b53D9277642d899DF5C87A3966A349A798F224", "4200000000000000000000000000000000000006", "833589fCD6eDb6E08f4c7C32D4f71b54bdA02913", 500); // OK
    test_pool_out!(test_sushiswapv3_out, SushiSwapV3, "57713F7716e0b0F65ec116912F834E49805480d2", "4200000000000000000000000000000000000006", "833589fCD6eDb6E08f4c7C32D4f71b54bdA02913", 500); // OK
    test_pool_out!(test_pancakeswapv3_out, PancakeSwapV3, "B775272E537cc670C65DC852908aD47015244EaF", "4200000000000000000000000000000000000006", "833589fCD6eDb6E08f4c7C32D4f71b54bdA02913", 500); // OK


    test_pool_out!(test_alienbasev3_out, AlienBaseV3, "B27f110571c96B8271d91ad42D33A391A75E6030", "4200000000000000000000000000000000000006", "833589fCD6eDb6E08f4c7C32D4f71b54bdA02913", 750);  
    test_pool_out!(test_dackieswapv3_out, DackieSwapV3, "fCD3960075c00af339A4E26afC76b949E5Ff06Ec", "4200000000000000000000000000000000000006", "833589fCD6eDb6E08f4c7C32D4f71b54bdA02913", 500);
    test_pool_out!(test_swapbasedv3_out, SwapBasedV3, "8D4B74fe1dfa2789CAa367F670eB4AC202107635", "4200000000000000000000000000000000000006", "833589fCD6eDb6E08f4c7C32D4f71b54bdA02913", 500);
    test_pool_out!(test_baseswapv3_out, BaseSwapV3, "74cb6260Be6F31965C239Df6d6Ef2Ac2B5D4f020", "4200000000000000000000000000000000000006", "833589fCD6eDb6E08f4c7C32D4f71b54bdA02913", 80); 

    // AERODROME VARIANTS
    // ------------------
    test_pool_out!(test_aerodrome_out, Aerodrome, "cDAC0d6c6C59727a65F871236188350531885C43", "4200000000000000000000000000000000000006", "833589fCD6eDb6E08f4c7C32D4f71b54bdA02913", 0); // OK
    test_pool_out!(test_slipstream_out, Slipstream, "b2cc224c1c9feE385f8ad6a55b4d94E92359DC59", "4200000000000000000000000000000000000006", "833589fCD6eDb6E08f4c7C32D4f71b54bdA02913", 100); // OK



    // MAVERICK VARIANTS
    //--------------------------
    test_pool_out!(test_maverickv1_out, MaverickV1, "3cfCc73dD7a81e5373CD9D50960D5bA5f113Cb7E", "50c5725949A6F0c72E6C4a641F24049A917DB0Cb", "833589fCD6eDb6E08f4c7C32D4f71b54bdA02913", 0);
    test_pool_out!(test_maverickv2_out, MaverickV2, "3cfCc73dD7a81e5373CD9D50960D5bA5f113Cb7E", "50c5725949A6F0c72E6C4a641F24049A917DB0Cb", "833589fCD6eDb6E08f4c7C32D4f71b54bdA02913", 0);


    // CURVE VARIANTS
    // ---------------------------------
    test_pool_out!(test_curve_two_out, CurveTwoCrypto, "749ef4ab10aef61151e14c9336b07727ffa5a323", "833589fcd6edb6e08f4c7c32d4f71b54bda02913", "8ee73c484a26e0a5df2ee2a4960b789967dd0415", 0);
    test_pool_out!(test_curve_tri_out, CurveTriCrypto, "6e53131f68a034873b6bfa15502af094ef0c5854", "417ac0e078398c154edfadd9ef675d30be60af93", "236aa50979d5f3de3bd1eeb40e81137f22ab794b", 0);

    // BALANCER VARIANTS
    // ------------------------
    test_pool_out!(test_balancer_out, BalancerV2, "6B1BaA0A046C9eBB7Da8668B66E9282A896E1Edb", "4200000000000000000000000000000000000006", "d9aAEc86B65D86f6A7B5B1b0c42FFA531710b6CA", 0);
}



// use the onchain quoters to get the amount out for a single swap
pub async fn onchain_quote(
    swap_path: &SwapPath,
    pool_type: PoolType,
    market_state: Arc<MarketState>, 
) -> U256 {
    dotenv::dotenv().ok();
    let swap_step = swap_path.steps.get(0).unwrap();

    let amount_in = U256::from(1e16);

    if pool_type.is_v2() {
        if pool_type == PoolType::Aerodrome {
            onchain_aerodrome(swap_step, amount_in).await
        } else {
            onchain_v2(swap_step, pool_type, amount_in, market_state).await
        }
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
    swap_path: &SwapPath,
    market_state: Arc<MarketState>,
) -> U256 {
    dotenv::dotenv().ok();
    let calculator = Calculator::new(market_state).await;
    calculator.calculate_output(swap_path)
}





// ONCHAIN QUOTERSK
// --------------------------
// Get the onchain quote for a v2 pool
pub async fn onchain_v2(swap_step: &SwapStep, pool_type: PoolType, amount_in: U256, market_state: Arc<MarketState>) -> U256 {
    let provider = ProviderBuilder::new().on_http(std::env::var("FULL").unwrap().parse().unwrap());

    let address = match pool_type {
        //PoolType::UniswapV2 => address!("4752ba5dbc23f44d87826276bf6fd6b1c372ad24"),
        PoolType::UniswapV2 => address!("7a250d5630B4cF539739dF2C5dAcb4c659F2488D"),
        PoolType::SushiSwapV2 => address!("6BDED42c6DA8FBf0d2bA55B2fa120C5e0c8D7891"),
        PoolType::PancakeSwapV2 => address!("8cFe327CEc66d1C090Dd72bd0FF11d690C33a2Eb"),
        PoolType::BaseSwapV2 => address!("327Df1E6de05895d2ab08513aaDD9313Fe505d86"),
        PoolType::SwapBasedV2 => address!("aaa3b1F1bd7BCc97fD1917c18ADE665C5D31F066"), 
        PoolType::DackieSwapV2 => address!("Ca4EAa32E7081b0c4Ba47e2bDF9B7163907Fe56f"), 
        PoolType::AlienBaseV2 => address!("8c1A3cF8f83074169FE5D7aD50B978e1cD6b37c7"),
        _ => panic!("will not reach here"),
    };

    sol!(
        #[sol(rpc)]
        contract Uniswap {
            function getAmountsOut(uint amountIn, address[] memory path) external view returns (uint[] memory amounts);
        }
    );

    let code = provider.get_code_at(address).await.unwrap();

    let router = AccountInfo {
        balance: U256::ZERO,
        nonce: 1_u64,
        code_hash: Bytecode::new_raw(code.clone()).hash_slow(),
        code: Some(Bytecode::new_raw(code)),
    };
    market_state.db.write().unwrap().insert_account_info(address, router);

    let calldata = Uniswap::getAmountsOutCall {
        amountIn: amount_in,
        path: vec![swap_step.token_in, swap_step.token_out],
    }.abi_encode();


    let mut evm = Evm::builder()
        .with_db(market_state.db.read().unwrap().clone())
        .modify_tx_env(|tx| {
            tx.caller = address!("0000000000000000000000000000000000000001");
            tx.transact_to = TransactTo::Call(address);
            tx.data = calldata.into();
            tx.value = U256::ZERO;
        }).build();

    
    let ref_tx = evm.transact().unwrap();
    println!("{:?}", ref_tx);
    let result = ref_tx.result; 

    println!("{:?}", result);
    match result {
        ExecutionResult::Success {
            output: value,
            ..
        } => {
            return U256::ZERO;
        }
        _=> U256::ZERO


    }


    /* 
    let contract = V2Router::new(address, provider);
    let V2Router::getAmountsOutReturn { amounts } = contract
        .getAmountsOut(amount_in, vec![swap_step.token_in, swap_step.token_out])
        .call()
        .await
        .unwrap();
    return *amounts.last().unwrap();
    */
}


// Get the onchain quote for a v3 pool
pub async fn onchain_v3(swap_step: &SwapStep, pool_type: PoolType, amount_in: U256) -> U256 {
    match pool_type {
        PoolType::UniswapV3 | PoolType::SushiSwapV3 | PoolType::PancakeSwapV3 | PoolType::Slipstream => {
            onchain_v3_quoter(pool_type, swap_step, amount_in).await
        }
        _ => onchain_v3_router(pool_type, swap_step, amount_in)
    }
}

// V3 amount out from the quoter
async fn onchain_v3_quoter(pool_type: PoolType, swap_step: &SwapStep, amount_in: U256 ) -> U256 {
    let provider = ProviderBuilder::new().on_http(std::env::var("FULL").unwrap().parse().unwrap());

    let address = match pool_type {
        PoolType::UniswapV3 => address!("3d4e44Eb1374240CE5F1B871ab261CD16335B76a"),
        PoolType::PancakeSwapV3 => address!("B048Bbc1Ee6b733FFfCFb9e9CeF7375518e25997"),
        PoolType::SushiSwapV3 => address!("b1E835Dc2785b52265711e17fCCb0fd018226a6e"),
        PoolType::Slipstream => address!("254cF9E1E6e233aa1AC962CB9B05b2cfeAaE15b0"),
        _ => panic!("Invalid pool type"),
    };

    if pool_type == PoolType::Slipstream {
        // Query the tickSpacing from the pool
        let pool_contract = SlipstreamPool::new(swap_step.pool_address, provider.clone());
        let SlipstreamPool::tickSpacingReturn { _0: tick_spacing } = pool_contract.tickSpacing().call().await.unwrap();

        let params =V3QuoterSlipstream::QuoteExactInputSingleParams {
            tokenIn: swap_step.token_in,
            tokenOut: swap_step.token_out,
            tickSpacing: tick_spacing.try_into().unwrap(),
            amountIn: amount_in,
            sqrtPriceLimitX96: U160::ZERO,
        };
        let contract = V3QuoterSlipstream::new(address, provider.clone());
        let V3QuoterSlipstream::quoteExactInputSingleReturn { amountOut , ..} = contract
            .quoteExactInputSingle(params)
            .call()
            .await
            .unwrap();
        return amountOut;
    } else {
        let params = V3Quoter::QuoteExactInputSingleParams {
            tokenIn: swap_step.token_in,
            tokenOut: swap_step.token_out,
            fee: swap_step.fee.try_into().unwrap(),
            amountIn: amount_in,
            sqrtPriceLimitX96: U160::ZERO,
        };
        let contract = V3Quoter::new(address, provider.clone());
        let V3Quoter::quoteExactInputSingleReturn { amountOut , ..} = contract
            .quoteExactInputSingle(params)
            .call()
            .await
            .unwrap();
        return amountOut;
    }

}


// V3 amount out from the router
fn onchain_v3_router(pool_type: PoolType, swap: &SwapStep, amount_in: U256) -> U256 {
    // setup the db
    let mut db = BlockStateDB::new(EmptyDB::new());

    /* 
    // addresses
    //let weth = address!("4200000000000000000000000000000000000006");
    let weth = address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");
    let account = address!("1E0294b6e4D72857B5eC467f5c2E52BDA37CA5b8");
    let router = match pool_type {
        PoolType::DackieSwapV3 => address!("195FBc5B8Fbd5Ac739C1BA57D4Ef6D5a704F34f7"),
        PoolType::SwapBasedV3 => address!("756C6BbDd915202adac7beBB1c6C89aC0886503f"),
        PoolType::AlienBaseV3 => address!("B20C411FC84FBB27e78608C24d0056D974ea9411"),
        PoolType::BaseSwapV3 => address!("1B8eea9315bE495187D873DA7773a874545D9D48"),
        _ => panic!("will not reach here")
    };

    // give account some weth
    let weth_balance_slot = U256::from(3);
    let one_ether = U256::from(1_000_000_000_000_000_000u128);
    let hashed_acc_balance_slot = keccak256((account, weth_balance_slot).abi_encode());
    db.insert_account_storage(weth, hashed_acc_balance_slot.into(), one_ether)
        .unwrap();
    let acc_info = AccountInfo {
        nonce: 0_u64,
        balance: one_ether,
        code_hash: keccak256(Bytes::new()),
        code: None,
    };
    db.insert_account_info(account, acc_info);


    // evm
    let mut evm = Evm::builder()
    .with_db(db)
    .modify_tx_env(|tx| {
        tx.caller = account;
        tx.transact_to = TransactTo::Call(weth);
        tx.value = U256::ZERO;
    }).build();

    // approve router to spend the weth
    let calldata = ERC20::approveCall {
        spender: router,
        amount: U256::from(1e18)
    }.abi_encode();
    evm.tx_mut().data = calldata.into();
    let res = evm.transact_commit().unwrap();

    // generate the calldata 
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

    evm.tx_mut().data = calldata.into();
    evm.tx_mut().transact_to = TransactTo::Call(router);
    let ref_tx = evm.transact().unwrap();
    let result = ref_tx.result;
    match result {
        ExecutionResult::Success {
            output: value,
            ..
        } => {
            match <U256>::abi_decode(&value.data(), false) {
                Ok(output) => output, 
                Err(_) => U256::ZERO
            }
        }
        _=> U256::ZERO
    }
    */
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
    let provider = Arc::new(ProviderBuilder::new()
        .on_http(std::env::var("FULL").unwrap().parse().unwrap()));
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

    let mut db = BlockStateDB::new(EmptyDB::new());

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
}

/* 
pub fn onchain_aerodrome(swap_step: &SwapStep, amount_in: U256) -> U256 {

    sol!(
        contract Aerodrome {
            function getAmountOut(uint256 amountIn, address tokenIn) external view returns (uint256);
        }
    );

    //let mut db = CacheDB::new(RethDB::new());
    let db = RethDB::new();

    let calldata = Aerodrome::getAmountOutCall {
        amountIn: amount_in,
        tokenIn: swap_step.token_in
    }.abi_encode();

    let mut evm = Evm::builder()
        .with_db(db)
        .modify_tx_env(|tx| {
            tx.caller = address!("0000000000000000000000000000000000000001");
            tx.transact_to = TransactTo::Call(swap_step.pool_address);
            tx.data = calldata.into();
            tx.value = U256::ZERO;
        }).build();

    
    let ref_tx = evm.transact().unwrap();
    let result = ref_tx.result; 
    match result {
        ExecutionResult::Success {
            output: value,
            ..
        } => {
            let a = match <U256>::abi_decode(&value.data(), false) {
                Ok(a) => a,
                Err(_) => U256::ZERO
            };
            return a;
        }
        _=> U256::ZERO


    }
}
*/

pub async fn onchain_aerodrome(swap_step: &SwapStep, amount_in: U256) -> U256 {
    sol!(
        #[sol(rpc)]
        contract Aerodrome {
            function getAmountOut(uint256 amountIn, address tokenIn) external view returns (uint256);
        }
    );

    let provider = ProviderBuilder::new().on_http(std::env::var("FULL").unwrap().parse().unwrap());
    let contract = Aerodrome::new(swap_step.pool_address, provider);

    let Aerodrome::getAmountOutReturn { _0: amount_out} = contract.getAmountOut(
        amount_in,
        swap_step.token_in
    ).call().await.unwrap();
    amount_out
}
