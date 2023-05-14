use clap::{Parser, ValueEnum};

use rpsl::expr::MpFilterExpr;

/// An IRR query and filter generation toolset.
#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// IRRd server hostname or IP address.
    #[arg(short = 'H', long, default_value = "whois.radb.net")]
    host: String,

    /// IRRd server port.
    #[arg(short = 'P', long, default_value_t = 43)]
    port: u16,

    /// Set the logging level.
    #[arg(short, long, default_value_t = log::LevelFilter::Warn)]
    log_level: log::LevelFilter,

    /// Output format.
    #[arg(short, long, value_enum, default_value = "plain")]
    format: Format,

    /// RPSL mp-filter expression to evaluate.
    filter: MpFilterExpr,
}

impl Args {
    /// Get the IRRd server hostname.
    #[must_use]
    pub fn host(&self) -> &str {
        &self.host
    }

    /// Get the IRRd server port number.
    #[must_use]
    pub const fn port(&self) -> u16 {
        self.port
    }

    /// Get object to query.
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    pub fn filter(self) -> MpFilterExpr {
        self.filter
    }

    /// Get log level.
    #[must_use]
    pub const fn log_level(&self) -> log::LevelFilter {
        self.log_level
    }
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum Format {
    Plain,
}
