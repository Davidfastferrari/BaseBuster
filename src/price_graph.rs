use crate::edge::{Edge, EdgeCalc};
use crate::path::Path;
use crate::types::{Exchange, Token};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

pub struct PriceGraph {
    edges: HashMap<Edge, Arc<dyn EdgeCalc>>,
    pre_built_paths: Vec<Path>,
    token_pairs: HashMap<(Token, Token), Vec<Edge>>,
}

impl PriceGraph {
    pub fn new() -> Self {
        Self {
            edges: HashMap::new(),
            pre_built_paths: Vec::new(),
            token_pairs: HashMap::new(),
        }
    }

    pub fn add_edge(
        &mut self,
        token_in: Token,
        token_out: Token,
        exchange: Exchange,
        fee: u16,
        edge: Arc<dyn EdgeCalc>,
    ) {
        let edge_id = Edge {
            token_in,
            token_out,
            exchange,
            fee,
        };
        self.edges.insert(edge_id, edge);
        self.token_pairs
            .entry((token_in, token_out))
            .or_default()
            .push(edge_id);
        self.token_pairs
            .entry((token_out, token_in))
            .or_default()
            .push(edge_id);
    }

    pub fn set_block_number(&mut self) {
        //  this can be something that maybe like pulls in the most volume tokens
        self.update_pre_built_paths();
    }

    fn update_pre_built_paths(&mut self) {
        self.pre_built_paths = self.generate_paths();
    }

    fn generate_paths(&self) -> Vec<Path> {
        let mut paths = Vec::new();
        let tokens: HashSet<_> = self
            .token_pairs
            .keys()
            .flat_map(|&(a, b)| vec![a, b])
            .collect();

        for &start_token in &tokens {
            paths.extend(self.generate_paths_for_token(start_token));
        }
        paths
    }

    fn generate_paths_for_token(&self, start: Token) -> Vec<Path> {
        let mut paths = Vec::new();

        // Iterate through all token pairs that start with our start token
        for (&(token_a, token_b), forward_edges) in &self.token_pairs {
            if token_a == start {
                // Check if there's a reverse path
                if let Some(backward_edges) = self.token_pairs.get(&(token_b, token_a)) {
                    // For each combination of forward and backward edge, create a reflexive path
                    for &forward_edge_id in forward_edges {
                        for &backward_edge_id in backward_edges {
                            paths.push(Path::Reflexive {
                                path: [
                                    (start, token_b),
                                    (token_b, start),
                                ],
                                swap_id: forward_edge_id,
                            });
                        }
                    }
                }
            }
        }

        paths
    }
}

/*
use crate::path::*;
use crate::types::*;
use alloy::primitives::{Address, U128, U256};
use pool_sync::Pool;

/// Unique edge identifier
type EdgeId = u32;

/// A graph edge (weight, exchange)
pub enum Edge {
    /// A edge for a uniswapV2 pool
    UniswapV2 {
        reserve_in: U128,
        reserve_out: U128,
        fee: u16,
        exchange_id: ExchangeId,
    },
    /// A edge of a uniswapV3 pool
    UniswapV3 {
        // sqrt price ratio x 2**96
        sqrt_p_x96: U256,
        liquidity: U256,
        fee: u16,
        /// Is this edge a token0 => token1 trade
        zero_for_one: bool,
        exchange_id: ExchangeId,
    },
}

impl Edge {
    /// quick edge hash
    /// a - token in
    /// b - token out
    /// c - exchange id
    /// d - pool fee (0 for v2 edges)
    pub fn hash(a: u8, b: u8, c: u8, fee: u16) -> u32 {
        // 8bit in | 8bit out | 8bit exchange | 16bit (fee)
        ((a & 63_u8) as u32)
            | (((b & 63_u8) as u32) << 5)
            | (((c & 63_u8) as u32) << 10)
            | ((fee as u32) << 16)
    }

    /// Get unique id of the edge
    /// Token in | token out | exchange id | fee
    pub fn id(&self, token_in: Token, token_out: Token) -> EdgeId {
        match self {
            Edge::UniswapV2 {
                exchange_id, fee, ..
            } => Edge::hash(token_in as u8, token_out as u8, *exchange_id as u8, *fee),
            Edge::UniswapV3 {
                exchange_id, fee, ..
            } => Edge::hash(token_in as u8, token_out as u8, *exchange_id as u8, *fee),
        }
    }
}

pub struct PriceGraph {
    /// Edges touched during a round of price updates.
    touched: bool,
}

impl PriceGraph {
    /// Returns true if the price graph has been updated
    pub fn touched(&self) -> bool {
        self.touched
    }

    /// Find supported arbitrage paths for token `start` through the provided pairs list
    /// This is intended to be run once to produce searchable paths for `find_arb`
    fn find_arbitrage_paths(start: Token, pairs: &[Pair]) -> Vec<Path> {
        // Build the graph
        let mut graph: HashMap<Token, Vec<Edge>> = HashMap::new();
        for pair in pairs {
            let edge_a = Edge {
                to: pair.token_b,
                exchange: pair.exchange,
                fee: pair.fee,
            };
            let edge_b = Edge {
                to: pair.token_a,
                exchange: pair.exchange,
                fee: pair.fee,
            };
            graph.entry(pair.token_a).or_default().push(edge_a);
            graph.entry(pair.token_b).or_default().push(edge_b);
        }

        let mut paths = Vec::new();

        // Find reflexive paths
        if let Some(edges) = graph.get(&start) {
            for edge1 in edges {
                if let Some(back_edges) = graph.get(&edge1.to) {
                    for edge2 in back_edges {
                        if edge2.to == start && edge1.exchange != edge2.exchange {
                            paths.push(Path::Reflexive {
                                start,
                                mid: edge1.to,
                                exchange1: edge1.exchange,
                                exchange2: edge2.exchange,
                                fee1: edge1.fee,
                                fee2: edge2.fee,
                            });
                        }
                    }
                }
            }
        }

        // Find triangular paths
        if let Some(edges) = graph.get(&start) {
            for edge1 in edges {
                if let Some(second_edges) = graph.get(&edge1.to) {
                    for edge2 in second_edges {
                        if edge2.to != start {
                            if let Some(third_edges) = graph.get(&edge2.to) {
                                for edge3 in third_edges {
                                    if edge3.to == start
                                        && edge1.exchange != edge3.exchange
                                        && edge1.fee + edge2.fee + edge3.fee < 150
                                    {
                                        // Example: total fee < 1.5%
                                        paths.push(Path::Triangular {
                                            start,
                                            mid1: edge1.to,
                                            mid2: edge2.to,
                                            exchange1: edge1.exchange,
                                            exchange2: edge2.exchange,
                                            exchange3: edge3.exchange,
                                            fee1: edge1.fee,
                                            fee2: edge2.fee,
                                            fee3: edge3.fee,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        paths
    }
}
*/
