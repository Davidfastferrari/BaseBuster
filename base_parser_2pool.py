import itertools
import os
from typing import Any, Dict

import networkx as nx
import ujson
import web3
from eth_typing import ChecksumAddress
from eth_utils.address import to_checksum_address

CHAIN = "base"
DATA_DIR = "/home/btd/code/cryo_data"
ARB_PATH = f"{CHAIN}_arbs_2pool.json"
WETH_ADDRESS = "0x4200000000000000000000000000000000000006"

BLACKLISTED_TOKENS = [
    # "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913",  # USDC
    # "0xfde4C96c8593536E31F229EA8f37b2ADa2699bb2",  # USDT
    # "0x50c5725949A6F0c72E6C4a641F24049A917DB0Cb",  # DAI
    # "0xd9aAEc86B65D86f6A7B5B1b0c42FFA531710b6CA",  # USDbC
]

lp_data: Dict[ChecksumAddress, Dict[str, Any]] = {}
for file_path in [
    os.path.join(DATA_DIR, f"{CHAIN}_lps_uniswap_v2.json"),
    os.path.join(DATA_DIR, f"{CHAIN}_lps_uniswap_v3.json"),
    os.path.join(DATA_DIR, f"{CHAIN}_lps_pancakeswap_v3.json"),
    os.path.join(DATA_DIR, f"{CHAIN}_lps_sushiswap_v2.json"),
    os.path.join(DATA_DIR, f"{CHAIN}_lps_sushiswap_v3.json"),
    os.path.join(DATA_DIR, f"{CHAIN}_lps_swapbased_v2.json"),
]:
    with open(file_path) as file:
        pools = ujson.load(file)
    for pool in pools:
        try:
            lp_data[pool["pool_address"]] = pool
        except KeyError:
            continue
    print(f"Found {len(pools)} pools in {file_path}")

# Build a multi-edge graph with tokens as nodes and liquidity pools as edges
G = nx.MultiGraph()
for pool in lp_data.values():
    G.add_edge(
        pool["token0"],
        pool["token1"],
        lp_address=pool["pool_address"],
        pool_type=pool["type"],
    )

G.remove_nodes_from(
    [to_checksum_address(token) for token in BLACKLISTED_TOKENS]
)
print(f"Graph ready: {len(G.nodes)} nodes, {len(G.edges)} edges")
print(f"Found {len(G[WETH_ADDRESS])} tokens with a WETH pair")

print("*** Finding two-pool arbitrage paths ***")
two_pool_arb_paths = {}
for token in G.neighbors(WETH_ADDRESS):
    pools = G.get_edge_data(token, WETH_ADDRESS).values()

    # skip tokens with only one pool
    if len(pools) < 2:
        continue

    for pool_a, pool_b in itertools.permutations(pools, 2):
        pool_a_address = pool_a["lp_address"]
        pool_b_address = pool_b["lp_address"]

        two_pool_arb_paths[id] = {
            "id": (
                id := web3.Web3.keccak(
                    hexstr="".join(
                        [
                            pool_a_address[2:],
                            pool_b_address[2:],
                        ]
                    )
                ).hex()
            ),
            "pools": {
                pool_a_address: lp_data[pool_a_address],
                pool_b_address: lp_data[pool_b_address],
            },
            "arb_types": ["cycle"],
            "path": [
                pool_a_address,
                pool_b_address,
            ],
        }

print(
    f"Found {len(two_pool_arb_paths)} unique two-pool arbitrage paths"
)

print("• Saving arb paths to JSON")
with open(ARB_PATH, "w") as file:
    ujson.dump(two_pool_arb_paths, file, indent=2)
