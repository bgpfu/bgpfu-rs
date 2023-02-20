//! Support library for `bgpfu`.
#![doc(html_root_url = "https://docs.rs/bgpfu/0.1.0-alpha.1")]
#![warn(missing_docs)]

/// Command line argument handling.
pub mod cli;
/// Command dispatchers.
pub mod cmd;

/// Collection of streams of IP prefixes into sets.
mod collect;
/// Query expression AST evaluation.
mod eval;
/// Query result output.
mod output;
/// Query pipelining and response handling.
mod query;
