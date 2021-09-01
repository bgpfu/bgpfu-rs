use std::io::{self, BufWriter};

use anyhow::Result;
use clap::Clap;

use bgpfu::{cli::Args, cmd::Dispatch};

fn main() -> Result<()> {
    // parse CLI args
    let args = Args::parse();
    // init logger
    stderrlog::new()
        .verbosity(args.verbosity())
        .timestamp(args.log_timestamp())
        .init()?;
    // // Lock stdout
    let stdout = io::stdout();
    let mut writer = BufWriter::new(stdout.lock());
    // dispatch to command
    args.command().dispatch(&mut writer)
}
