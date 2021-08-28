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

    /// Set the logging level.
    #[structopt(short, long, default_value = "warn")]
    log_level: log::LevelFilter,

    /// Filter output by address family.
    #[structopt(
        long,
        possible_values = &AddressFamilyFilter::VARIANTS,
        case_insensitive = true,
        default_value = "any"
    )]
    afi: AddressFamilyFilter,

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
    filter: FilterExpr,
}

impl Args {
    /// Get address family filter.
    pub fn address_family(&self) -> &AddressFamilyFilter {
        &self.afi
    }

    /// Get object to query.
    pub fn filter(&self) -> FilterExpr {
        self.filter.clone()
    }

    /// Get log level.
    pub fn log_level(&self) -> &log::LevelFilter {
        &self.log_level
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
