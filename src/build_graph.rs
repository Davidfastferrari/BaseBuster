use petgraph::algo::all_simple_paths;
use std::collections::HashMap;

pub fn construct_graph(
    pools: &Vec<Pool>,
    filtered_pools: &Vec<Pool>,
    token_to_index: &mut HashMap<Address, NodeIndex>,
) -> Graph<Address, (u128, u128), Undirected> {
    let mut graph = Graph::new_undirected();

    for pool in filtered_pools {
        let token0 = pool.token0_address();
        let token1 = pool.token1_address();

        let node0 = *token_to_index
            .entry(token0)
            .or_insert_with(|| graph.add_node(token0));
        let node1 = *token_to_index
            .entry(token1)
            .or_insert_with(|| graph.add_node(token1));

        let (reserve0, reserve1) = pool.get_reserves();
        graph.add_edge(node0, node1, (reserve0, reserve1));
    }

    graph
}

fn calculate_output(amount_in: u128, reserve_in: u128, reserve_out: u128) -> u128 {
    let amount_in_with_fee = amount_in * 997;
    let numerator = amount_in_with_fee * reserve_out;
    let denominator = reserve_in * 1000 + amount_in_with_fee;
    numerator / denominator
}

pub fn find_best_arbitrage_path(
    graph: &Graph<Address, (u128, u128), Undirected>,
    start_token: Address,
    input_amount: u128,
    max_depth: usize,
) -> Option<(Vec<NodeIndex>, u128)> {
    let start_node = graph.node_indices().find(|&n| graph[n] == start_token).unwrap();
    let mut best_path = None;
    let mut best_output = input_amount;

    fn dfs(
        graph: &Graph<Address, (u128, u128), Undirected>,
        current_node: NodeIndex,
        start_node: NodeIndex,
        current_amount: u128,
        current_path: Vec<NodeIndex>,
        depth: usize,
        max_depth: usize,
        best_path: &mut Option<(Vec<NodeIndex>, u128)>,
        best_output: &mut u128,
    ) {
        if depth > max_depth {
            return;
        }

        if current_node == start_node && depth > 0 {
            if current_amount > *best_output {
                *best_output = current_amount;
                *best_path = Some((current_path.clone(), current_amount));
            }
            return;
        }

        for neighbor in graph.neighbors(current_node) {
            let edge = graph.edge_weight(graph.find_edge(current_node, neighbor).unwrap()).unwrap();
            let (reserve_in, reserve_out) = if graph[current_node] < graph[neighbor] {
                *edge
            } else {
                (edge.1, edge.0)
            };

            let output_amount = calculate_output(current_amount, reserve_in, reserve_out);
            let mut new_path = current_path.clone();
            new_path.push(neighbor);

            dfs(
                graph,
                neighbor,
                start_node,
                output_amount,
                new_path,
                depth + 1,
                max_depth,
                best_path,
                best_output,
            );
        }
    }

    dfs(
        graph,
        start_node,
        start_node,
        input_amount,
        vec![start_node],
        0,
        max_depth,
        &mut best_path,
        &mut best_output,
    );

    best_path
}

let token_addresses: HashSet<Address> = [
    address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
    address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"),
    address!("dAC17F958D2ee523a2206206994597C13D831ec7"),
    address!("2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599"),
    address!("4c9EDD5852cd905f086C759E8383e09bff1E68B3"),
    address!("7f39C581F595B53c5cb19bD0b3f8dA6c935E2Ca0"),
    address!("6B175474E89094C44Da98b954EedeAC495271d0F"),
    address!("38E68A37E401F7271568CecaAc63c6B1e19130B4"),
    address!("8802269D1283cdB2a5a329649E5cB4CdcEE91ab6"),
    address!("Cd5fE23C85820F7B72D0926FC9b05b43E359b7ee"),
    address!("853d955aCEf822Db058eb8505911ED77F175b99e"),
    address!("D5f58b528f810dB3358B2aA52CBD93f94b3D8672"),
    address!("9f8F72aA9304c8B593d555F12eF6589cC3A579A2"),
    address!("f939E0A03FB07F59A73314E73794Be0E57ac1b4E"),
    address!("bf5495Efe5DB9ce00f80364C8B423567e58d2110"),
    address!("95aD61b0a150d79219dCF64E1E6Cc01f0B64C4cE"),
    address!("683A4ac99E65200921f556A19dADf4b0214B5938"),
    address!("43FD9De06bb69aD771556E171f960A91c42D2955"),
    address!("aaeE1A9723aaDB7afA2810263653A34bA2C21C7a"),
    address!("Be9895146f7AF43049ca1c1AE358B0541Ea49704"),
    address!("ae7ab96520DE3A18E5e111B5EaAb095312D7fE84"),
    address!("EeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE"),
    address!("6982508145454Ce325dDbE47a25d4ec3d2311933"),
    address!("63989348EBe7590b8863C5A242F7EE01EFcCa0c7"),
    address!("3fFEea07a27Fab7ad1df5297fa75e77a43CB5790"),
    address!("83F20F44975D03b1b09e64809B757c47f942BEeA"),
    address!("A1290d69c65A6Fe4DF752f95823fae25cB99e5A7"),
    address!("Efa21Ee9000Ee9ebfcA048B6d097A7ac746C1946"),
    address!("36C7188D64c44301272Db3293899507eabb8eD43"),
    address!("D29DA236dd4AAc627346e1bBa06A619E8c22d7C5"),
    address!("576e2BeD8F7b46D34016198911Cdf9886f78bea7"),
    address!("68BbEd6A47194EFf1CF514B50Ea91895597fc91E"),
    address!("9f4aC5eA89B1623E35328904D7D61B08B55dAA50"),
    address!("F5f97246f70EB4A84a67a72eEE8D7c9312018AA4"),
    address!("ae78736Cd615f374D3085123A210448E74Fc6393"),
    address!("9048Bc764cB7c40EA7162540A1F74fAD093b57fE"),
    address!("E39f5C9B6a9c225a50e1Bb3b83649aE85BDF0591"),
    address!("9D39A5DE30e57443BfF2A8307A4256c8797A3497"),
    address!("57e114B691Db790C35207b2e685D4A43181e6061"),
    address!("9E6be44cC1236eEf7e1f197418592D363BedCd5A"),
    address!("77E06c9eCCf2E797fd462A92B6D7642EF85b0A44"),
    address!("5fAa989Af96Af85384b8a938c2EdE4A7378D9875"),
    address!("36B25341b2Ff1BBc1b019B041EC7423A6Cb21969"),
    address!("Fe0c30065B384F05761f15d0CC899D4F9F9Cc0eB"),
    address!("6135177A17E02658dF99A07A2841464deB5B8589"),
    address!("40D16FC0246aD3160Ccc09B8D0D3A2cD28aE6C2f"),
    address!("F16C6F26CcbC394a8B8647dea728Dd90D1Bde433"),
    address!("3c3a81e81dc49A522A592e7622A7E711c06bf354"),
    address!("194605Aa511D90F1CBFEc2d6aA61c3D713F7157E"),
    address!("790814Cd782983FaB4d7B92CF155187a865d9F18"),
    address!("874F263c085Ef99BE6976347D0a265065E49641C"),
    address!("000000000C08b9884B846Dfc7cc7ff74D01F9513"),
    address!("FAe103DC9cf190eD75350761e95403b7b8aFa6c0"),
    address!("bc22322BA299d490974bE3c3d258E43BABdB7311"),
    address!("07040042Ca6f9c12bAEC45bF13c20e9B1FD1C66F"),
    address!("f819d9Cb1c2A819Fd991781A822dE3ca8607c3C9"),
    address!("fAbA6f8e4a5E8Ab82F62fe7C39859FA577269BE3"),
    address!("582d872A1B094FC48F5DE31D3B73F2D9bE47def1"),
    address!("2eC6603cff128Dd4500e6edc90cF4a25050f37b3"),
    address!("D9A442856C234a39a81a089C06451EBAa4306a72"),
    address!("622b6330F226bF08427dcad49c9eA9694604BF2D"),
    address!("B84fEAeec76a0Fb2C52E167E49E4246d520C9d15"),
    address!("767FE9EDC9E0dF98E07454847909b5E959D7ca0E"),
    address!("6E79B51959CF968d87826592f46f819F92466615"),
    address!("6De037ef9aD2725EB40118Bb1702EBb27e4Aeb24"),
    address!("5508E2696885b99e563CaF8bE1F706f4E1E42Cab"),
    address!("8E3f2543F946a955076c137700eaD4c9439e7fcA"),
    address!("8eD97a637A790Be1feff5e888d43629dc05408F6"),
    address!("67466BE17df832165F8C80a5A120CCc652bD7E69"),
    address!("c770EEfAd204B5180dF6a14Ee197D99d808ee52d"),
    address!("85bDAeE8319400e5713558089a2E750C6A425797"),
    address!("d5F7838F5C461fefF7FE49ea5ebaF7728bB0ADfa"),
    address!("F73328afb07E9F9950747B84aBD91a52C7AF3CE1"),
    address!("514910771AF9Ca656af840dff83E8264EcF986CA"),
    address!("cf0C122c6b73ff809C693DB761e7BaeBe62b6a2E"),
    address!("7bb08a75ea1D11298c6A3fF2A638153C96792C93"),
    address!("636bd98fC13908e475F56d8a38a6e03616Ec5563"),
    address!("3927FB89F34BbeE63351A6340558Eebf51A19FB8"),
    address!("D31a59c85aE9D8edEFeC411D448f90841571b89c"),
    address!("73A15FeD60Bf67631dC6cd7Bc5B6e8da8190aCF5"),
    address!("DDB3422497E61e13543BeA06989C0789117555c5"),
    address!("7Fc66500c84A76Ad7e9c93437bFc5Ac33E2DDaE9"),
    address!("A36A8fc6610A6d47D33B5EB53718703bd6b1e7f7"),
    address!("D7823B4753c68B7C55927E331f8eB2661eb10B72"),
    address!("aea46A60368A7bD060eec7DF8CBa43b7EF41Ad85"),
    address!("614Da3b37B6F66F7Ce69B4Bbbcf9a55CE6168707"),
    address!("58cB30368ceB2d194740b144EAB4c2da8a917Dcb"),
    address!("fefe157c9d0aE025213092ff9a5cB56ab492BaB8"),
    address!("32B053F2CBA79F80ada5078cb6b305da92BDe6e1"),
    address!("14feE680690900BA0ccCfC76AD70Fd1b95D10e16"),
    address!("b2617246d0c6c0087f18703d576831899ca94f01"),
    address!("4704cF8D968aA0AF61ed7Cf8F2DE0d0B31cAb623"),
    address!("594DaaD7D77592a2b97b725A7AD59D7E188b5bFa"),
    address!("D533a949740bb3306d119CC777fa900bA034cd52"),
    address!("A35923162C49cF95e6BF26623385eb431ad920D3"),
    address!("808507121B80c02388fAd14726482e061B8da827"),
    address!("7613C48E0cd50E42dD9Bf0f6c235063145f6f8DC"),
    address!("1f9840a85d5aF5bf1D1762F925BDADdC4201F984"),
    address!("18b3236D6C6C1A79522696Ee70b1c28932D8902F"),
]
.iter()
.cloned()
.collect();