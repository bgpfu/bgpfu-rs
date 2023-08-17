use bgpfu::RpslEvaluator;

use clap::Parser;

use clap_verbosity_flag::Verbosity;

use ip::traits::PrefixSet as _;

use rpsl::expr::MpFilterExpr;

use simplelog::SimpleLogger;

use crate::Format;

/// Entry-point function for the `bgpfu` CLI tool.
#[allow(clippy::missing_errors_doc)]
pub fn main() -> anyhow::Result<()> {
    let args = Cli::parse();
    SimpleLogger::init(
        args.verbosity.log_level_filter(),
        simplelog::Config::default(),
    )?;
    RpslEvaluator::new(args.host(), args.port())?
        .evaluate(args.filter())?
        .ranges()
        .for_each(|range| println!("{range}"));
    Ok(())
}

/// An IRR query and filter generation toolset.
#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// IRRd server hostname or IP address.
    #[arg(short = 'H', long, default_value = "whois.radb.net")]
    host: String,

    /// IRRd server port.
    #[arg(short = 'P', long, default_value_t = 43)]
    port: u16,

    #[command(flatten)]
    verbosity: Verbosity,

    /// Output format.
    #[arg(short, long, value_enum, default_value_t = Format::Plain)]
    format: Format,

    /// RPSL mp-filter expression to evaluate.
    filter: MpFilterExpr,
}

impl Cli {
    /// Get the IRRd server hostname.
    #[must_use]
    fn host(&self) -> &str {
        &self.host
    }

    /// Get the IRRd server port number.
    #[must_use]
    const fn port(&self) -> u16 {
        self.port
    }

    /// Get object to query.
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    fn filter(self) -> MpFilterExpr {
        self.filter
    }
}
