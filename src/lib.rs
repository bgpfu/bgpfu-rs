//! Support library for `bgpfu`.
#![doc(html_root_url = "https://docs.rs/bgpfu/0.1.0-alpha.1")]
#![warn(missing_docs)]

#[macro_use]
extern crate pest_derive;

/// Command line argument handling.
pub mod cli;
/// Command dispatchers.
pub mod cmd;

/// Query expression AST.
mod ast;
/// Collection of streams of IP prefixes into sets.
mod collect;
/// Query result output.
mod output;
/// Query expression parser.
mod parser;
/// Query pipelining and response handling.
mod query;
