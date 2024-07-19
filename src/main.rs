use petgraph::dot::{Config, Dot};
use petgraph::{algo, prelude::*};
use rand::Rng;
use std::fs::File;
use std::io::Write;

fn main() {
    let mut graph = UnGraph::<&str, i32>::with_capacity(5, 6);
    let weth = graph.add_node("weth");
    let usdc = graph.add_node("usdc");
    let bonk = graph.add_node("bonk");
    let tromp = graph.add_node("tromp");
    let a = graph.add_node("a");
    let b = graph.add_node("b");
    let c = graph.add_node("c");
    let d = graph.add_node("d");
    let e = graph.add_node("e");
    let f = graph.add_node("f");
    let h = graph.add_node("h");
    let i = graph.add_node("h");
    let j = graph.add_node("h");
    let k = graph.add_node("h");
    let l = graph.add_node("h");
    let m = graph.add_node("h");
    let n = graph.add_node("h");
    let o = graph.add_node("h");
    let p = graph.add_node("h");








    let nodes = [weth, usdc, bonk, tromp, a, b, c, d, e, f, h, i, j, k, l, m, n, o, p];

    // Generate a large number of random edges
    let mut rng = rand::thread_rng();
    for _ in 0..200 {
        let from = nodes[rng.gen_range(0..nodes.len())];
        let to = nodes[rng.gen_range(0..nodes.len())];
        let weight = rng.gen_range(1..10);
        graph.add_edge(from, to, weight);
    }

    // Add some specific edges to ensure connectivity
    graph.extend_with_edges([
        (weth, usdc, 1),
        (weth, bonk, 1),
        (weth, tromp, 1),
        (tromp, usdc, 1),
        (usdc, bonk, 1),
        (usdc, weth, 2),
        (usdc, a, 2),
        (a, weth, 2),
        (b, c, 3),
        (c, d, 4),
        (d, e, 5),
        (e, f, 6),
        (f, h, 7),
        (h, weth, 8),
    ]);

    let cycles: Vec<Vec<NodeIndex>> =
        algo::all_simple_paths::<Vec<_>, _>(&graph, weth, weth, 0,Some( 3)).collect();
    //println!("cycles {}", cycles.len());

    /*
    println!("Cycles starting and ending at weth:");
    for (i, cycle) in cycles.iter().enumerate() {
        let cycle_str: Vec<String> = cycle
            .windows(2)
            .map(|pair| {
                let from = graph[pair[0]];
                let to = graph[pair[1]];
                let weight = graph
                    .edge_weight(graph.find_edge(pair[0], pair[1]).unwrap())
                    .unwrap();
                format!("{} --({})-> {}", from, weight, to)
            })
            .collect();
        println!("Cycle {}: {}", i + 1, cycle_str.join(" "));
    }
    */

    // Custom configuration to include edge weights
    /*
    let dot = Dot::with_config(&graph, &[Config::EdgeIndexLabel]);
    let mut file = File::create("graph.dot").expect("Could not create file");
    write!(file, "{:?}", dot).expect("Could not write to file");
    println!("DOT file 'graph.dot' has been generated.");
    */
}

