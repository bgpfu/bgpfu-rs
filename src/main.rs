use std::io::{self, BufWriter};

use anyhow::Result;
use structopt::StructOpt;

use bgpfu::{cli::Args, query::Resolver};

fn main() -> Result<()> {
    // parse CLI args
    let args = Args::from_args();
    // init logger
    stderrlog::new()
        .verbosity(args.verbosity())
        .timestamp(args.log_timestamp())
        .init()?;
    // parse RPSL filter expression
    let filter = args.filter()?;
    // resolve filter expression
    let sets = Resolver::new(&args)?.resolve(&filter)?;
    // write output
    let stdout = io::stdout();
    let mut writer = BufWriter::new(stdout.lock());
    args.format().write_prefix_sets(&sets, &mut writer)
}
