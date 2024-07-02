use pool_sync::Pool;


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
        pub fn find_paths(start: Token, Pools: &[Pool]) -> Vec<Path> {
                todo!()
        }
}