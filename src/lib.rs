//! Support library for `bgpfu`.
#![doc(html_root_url = "https://docs.rs/bgpfu/0.1.0-alpha.1")]
#![warn(missing_docs)]

#[macro_use]
extern crate pest_derive;

/// Command line argument handling.
pub mod cli;
/// Query result output.
pub mod output;
/// Query pipelining and response handling.
pub mod query;

/// Query expression AST.
mod ast;
/// Collection of streams of IP prefixes into sets.
mod collect;
/// Query expression parser.
mod parser;
