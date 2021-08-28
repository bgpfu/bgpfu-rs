use anyhow::Result;
use simple_logger::SimpleLogger;
use structopt::StructOpt;

use bgpfu::{cli::Args, query::Resolver};

fn main() -> Result<()> {
    let args = Args::from_args();
    SimpleLogger::new().with_level(*args.log_level()).init()?;
    let (ipv4_set, ipv6_set) = Resolver::new(&args)?.resolve(args.filter())?;
    if let Some(set) = ipv4_set {
        set.ranges().for_each(|range| println!("{}", range));
    }
    if let Some(set) = ipv6_set {
        set.ranges().for_each(|range| println!("{}", range));
    }
    Ok(())
}
