use clap::Parser;

use ip::traits::PrefixSet as _;

use simple_logger::SimpleLogger;

use bgpfu::{cli::Args, query::RpslEvaluator};

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    SimpleLogger::new().with_level(args.log_level()).init()?;
    RpslEvaluator::new(args.host(), args.port())?
        .evaluate(args.filter())?
        .ranges()
        .for_each(|range| println!("{range}"));
    Ok(())
}
