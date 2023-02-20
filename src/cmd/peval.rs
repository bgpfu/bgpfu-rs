use std::io::Write;

use anyhow::Result;
use clap::Clap;
use rpsl::{expr::FilterExpr, names::AutNum};
use strum::VariantNames;

use crate::{
    output::{Format, Formatter},
    query::{AddressFamilyFilter, Resolver},
};

use super::Dispatch;

#[derive(Clap, Debug)]
pub struct Peval {
    #[clap(from_global)]
    addr: String,

    /// Filter output by address family.
    #[clap(
        long,
        possible_values = &AddressFamilyFilter::VARIANTS,
        case_insensitive = true,
        default_value_t,
        global = true,
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
    /// $ bgpfu peval -R --peeras AS65000 'PeerAS^+ AND { 0.0.0.0/0^8-24, ::/0^16-48 }'
    #[clap(long, global = true)]
    peeras: Option<AutNum>,

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
    #[clap(global = true, default_value = "{}")]
    filter: String,

    #[clap(subcommand)]
    format: Format,
}

impl Peval {
    /// Get parsed filter expression.
    pub fn filter(&self) -> Result<FilterExpr> {
        Ok(self.filter.parse()?)
    }
}

impl<W: Write> Dispatch<W> for Peval {
    fn dispatch(&self, w: &mut W) -> Result<()> {
        let filter = self.filter()?;
        let mut resolver = Resolver::new(&self.addr, &self.afi, self.peeras.as_ref())?;
        let sets = resolver.resolve(&filter)?;
        self.format.write_prefix_sets(&sets, w)
    }
}
