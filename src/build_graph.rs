/*
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

*/
