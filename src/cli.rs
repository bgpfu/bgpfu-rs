use anyhow::Result;
use irrc::types::AutNum;
use structopt::{clap::AppSettings, StructOpt};
use strum::{EnumString, EnumVariantNames, VariantNames};

use crate::{ast::FilterExpr, query::AddressFamilyFilter};

/// An IRR query and filter generation toolset.
#[derive(StructOpt, Debug)]
#[structopt(
    rename_all = "kebab_case",
    after_help = "See '--help' for detailed usage information.\n",
    setting = AppSettings::ColoredHelp,
)]
pub struct Args {
    /// IRRd server hostname or IP address.
    #[structopt(short = "H", long, default_value = "whois.radb.net")]
    host: String,

    /// IRRd server port.
    #[structopt(short = "P", long, default_value = "43")]
    port: u16,

    /// Increase logging verbosity.
    ///
    /// This flag may be repeated to further increase logging detail.
    /// By default, logs are emitted at the WARNING level.
    #[structopt(short = "v", group = "verbosity", parse(from_occurrences))]
    verbosity_pos: usize,

    /// Decrease logging verbosity.
    ///
    /// This flag may be repeated to further decrease logging detail.
    /// By default, logs are emitted at the WARNING level.
    #[structopt(short = "q", group = "verbosity", parse(from_occurrences))]
    verbosity_neg: usize,

    /// Logging timestamp format.
    #[structopt(
        long,
        possible_values = &["sec", "ms", "us", "ns", "off"],
        default_value = "off",
    )]
    log_timestamp: stderrlog::Timestamp,

    /// Filter output by address family.
    #[structopt(
        long,
        possible_values = &AddressFamilyFilter::VARIANTS,
        case_insensitive = true,
        default_value = "any"
    )]
    afi: AddressFamilyFilter,

    /// RPSL `PeerAS` substitution value.
    ///
    /// RPSL filters allow the string `PeerAS` to appear as a place-holder for
    /// an `aut-num`, allowing a single filter expression to be re-used in the
    /// context of multiple peer ASNs.
    ///
    /// # Example
    ///
    /// Get IPv4 and IPv6 routes and more-specifics with `origin: AS65000`,
    /// within the given minimum/maximum prefix length bounds:
    ///
    /// $ bgpfu --peeras AS65000 'PeerAS^+ AND { 0.0.0.0/0^8-24, ::/0^16-48 }'
    #[structopt(long)]
    peeras: Option<AutNum>,

    /// Output format.
    #[structopt(
        short,
        long,
        possible_values = &Format::VARIANTS,
        case_insensitive = true,
        default_value = "plain"
    )]
    format: Format,

    /// RPSL filter expression to evaluate.
    ///
    /// An RPSL '<mp-filter>' expression, as defined in [RFC4012] and
    /// [RFC2622].
    ///
    /// Currently only expressions evaluating to an "Address-Prefix Set" are
    /// supported.
    ///
    /// [RFC2622]: https://datatracker.ietf.org/doc/html/rfc2622#section-5.4
    ///
    /// [RFC4012]: https://datatracker.ietf.org/doc/html/rfc4012#section-2.5.2
    filter: String,
}

impl Args {
    /// Get address family filter.
    pub fn address_family(&self) -> &AddressFamilyFilter {
        &self.afi
    }

    /// Get `PeerAS` substitution value.
    pub fn peeras(&self) -> Option<&AutNum> {
        self.peeras.as_ref()
    }

    /// Get parsed filter expression.
    pub fn filter(&self) -> Result<FilterExpr> {
        self.filter.parse()
    }

    /// Calculate logging verbosity.
    pub fn verbosity(&self) -> usize {
        1 + self.verbosity_pos - self.verbosity_neg
    }

    /// Get log timestamping option.
    pub fn log_timestamp(&self) -> stderrlog::Timestamp {
        self.log_timestamp
    }

    /// Construct socket address for IRR client connection.
    pub fn addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

#[derive(Debug, EnumString, EnumVariantNames)]
#[strum(serialize_all = "kebab_case")]
enum Format {
    Plain,
}
