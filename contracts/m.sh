#!/bin/bash
# Exit immediately if a command exits with a non-zero status
set -e

# Ethereum mainnet RPC URL (replace with your own if you have one)
# Start Anvil with a forked mainnet
echo "Starting Anvil with forked mainnet..."
anvil --fork-url "https://eth.merkle.io" &
ANVIL_PID=$!

# Give Anvil a moment to start up
sleep 2

# Set up variables
RPC_URL="http://localhost:8545"
PRIVATE_KEY="0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"  # Anvil's default private key
SENDER=$(cast wallet address $PRIVATE_KEY)

# Real token addresses
WETH="0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"
USDC="0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"
DAI="0x6B175474E89094C44Da98b954EedeAC495271d0F"

# Real DEX router addresses
UNIV2_ROUTER="0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D"
UNIV3_ROUTER="0xE592427A0AEce92De3Edee1F18E0157C05861564"
SUSHIV2_ROUTER="0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F"

echo "Deploying MultiExchangeSwap contract..."
# Deploy MultiExchangeSwap
MULTI_SWAP=$(forge create src/SwapQuoter.sol:MultiExchangeSwap --rpc-url $RPC_URL --private-key $PRIVATE_KEY --json | jq -r .deployedTo)
echo "MultiExchangeSwap deployed at: $MULTI_SWAP"

echo "Setting up initial token balances..."
# Get some WETH by depositing ETH
cast send $WETH "deposit()" --value 10000000000000000000 --rpc-url $RPC_URL --private-key $PRIVATE_KEY

# Approve MultiExchangeSwap to spend WETH
cast send $WETH "approve(address,uint256)" $MULTI_SWAP 10000000000000000000 --rpc-url $RPC_URL --private-key $PRIVATE_KEY

echo "Performing swap..."
# Manually construct the ABI-encoded swap steps
SWAP_STEPS="0x00000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000002000000000000000000000000${UNIV2_ROUTER:2}000000000000000000000000${WETH:2}000000000000000000000000${USDC:2}0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000${UNIV3_ROUTER:2}000000000000000000000000${USDC:2}000000000000000000000000${DAI:2}0000000000000000000000000000000000000000000000000000000000000001"

# Perform swap
SWAP_RESULT=$(cast send $MULTI_SWAP "swap((address,address,address,uint8)[],uint256)" $SWAP_STEPS 1000000000000000000 --rpc-url $RPC_URL --private-key $PRIVATE_KEY)

echo "Swap transaction hash: $SWAP_RESULT"

# Check final balance of DAI
FINAL_BALANCE=$(cast call $DAI "balanceOf(address)" $SENDER --rpc-url $RPC_URL)
echo "Final balance of DAI: $FINAL_BALANCE"

if [ "$FINAL_BALANCE" != "0" ]; then
    echo "Test passed: Received non-zero amount of DAI"
else
    echo "Test failed: Did not receive any DAI"
    exit 1
fi

# Kill Anvil process
kill $ANVIL_PID

echo "Test completed successfully!"
