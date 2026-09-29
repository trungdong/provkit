//! PROV-N (W3C Recommendation, 2013) read and write.

// The parser (next task) is the lexer's only client.
#[expect(dead_code)]
mod lexer;
mod write;

pub use write::{statement, to_string, write};
