use crate::types::Token;


// The number or tokens we are searching over
const NUM_TOKENS: usize = Token::VARIANT_COUNT;
const _: () assert!(N <= 64, "update pair idendity hash");



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


