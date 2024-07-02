

use crate::types::Token;
use crate::edge::Edge;

/// Struct of an arbitrage path
#[derive(Debug)]
pub enum Path {
    // Reflextive 2 hop path A/B -> B/A
    // Both hops will be different dexs
    Reflexive {
        path: [(Token, Token); 2], swap_id: Edge 
    },
    Triangle {
        path: [(Token, Token); 3], swap_id: Edge 
    }
}



























































/*
use crate::types::Token;


// The number or tokens we are searching over
// Make sure we are searching over less than 64 tokens
pub const NUM_TOKENS: usize = Token::VARIANT_COUNT;
const _: () = assert!(NUM_TOKENS <= 64, "update pair idendity hash");

// A reflexive path type. A/B -> B/A
pub type Reflexive = [(usize, usize); 2] ;

// A triangle path type. A/B -> B/C -> C/A
pub type Triangle = [(usize, usize); 3];

pub enum Path {
        //  Path with immediate neighbor from the start
        // base_id uniquily identifies the base edge
        Reflexive { path: Reflexive, base_id: u16 },
        //  Path with 2nd degree neighbor from the start
        // base_id uniquily identifies the base edge
        Triangle { path: Triangle, base_id: u16 },
}


*/
