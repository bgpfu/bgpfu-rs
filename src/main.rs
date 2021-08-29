use anyhow::Result;
use structopt::StructOpt;

use bgpfu::{cli::Args, query::Resolver};

fn main() -> Result<()> {
    let args = Args::from_args();
    stderrlog::new()
        .verbosity(args.verbosity())
        .timestamp(args.log_timestamp())
        .init()?;
    let filter = args.filter()?;
    let (ipv4_set, ipv6_set) = Resolver::new(&args)?.resolve(&filter)?;
    if let Some(set) = ipv4_set {
        set.ranges().for_each(|range| println!("{}", range));
    }
    if let Some(set) = ipv6_set {
        set.ranges().for_each(|range| println!("{}", range));
    }
    Ok(())
}
