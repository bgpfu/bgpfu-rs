use std::io::Write;

use anyhow::Result;
use clap::Clap;
use irrc::types::AutNum;
use num::Zero;
use prefixset::{IpPrefix, Ipv4Prefix, Ipv6Prefix, PrefixSet};

use super::Formatter;

trait IosAfiName {
    const AFI: &'static str;
}

impl IosAfiName for Ipv4Prefix {
    const AFI: &'static str = "ip";
}

impl IosAfiName for Ipv6Prefix {
    const AFI: &'static str = "ipv6";
}

#[derive(Clap, Debug, Default)]
pub struct CiscoIos {
    /// prefix-list name.
    ///
    /// Name of the Cisco IOS ip/ipv6 prefix-lists.
    ///
    /// Any occurrences the string `PeerAS` will be substituted for the value
    /// provided in the `--peeras` option.
    #[clap(short = 'l', long, default_value = "PeerAS")]
    prefix_list_name: String,

    #[clap(from_global)]
    peeras: Option<AutNum>,
}

impl CiscoIos {
    fn prefix_list_name(&self) -> String {
        if let Some(peeras) = self.peeras {
            self.prefix_list_name.replace("PeerAS", &peeras.to_string())
        } else {
            self.prefix_list_name.clone()
        }
    }

    fn write_prefix_set<P, W>(&self, set: &PrefixSet<P>, w: &mut W) -> Result<()>
    where
        P: IpPrefix + IosAfiName,
        W: Write,
    {
        let name = self.prefix_list_name();
        let afi = P::AFI;
        let max_length = P::MAX_LENGTH;
        writeln!(w, "no {} prefix-list {}", afi, name)?;
        let mut ranges = set.ranges().peekable();
        if ranges.peek().is_some() {
            ranges.try_for_each(|range| {
                write!(w, "{} prefix-list {} permit {}", afi, name, range.base())?;
                let length = range.base().length();
                let (lower, upper) = range.range().into_inner();
                if length < lower {
                    if upper < max_length {
                        writeln!(w, " ge {} le {}", lower, upper)
                    } else {
                        writeln!(w, " ge {}", lower)
                    }
                } else if length < upper {
                    writeln!(w, " le {}", upper)
                } else {
                    writeln!(w)
                }
            })
        } else {
            log::warn!("{} prefix-list {} is empty", afi, name);
            let default = P::new(P::Bits::zero(), 0)?;
            writeln!(
                w,
                "{} prefix-list {} deny {} le {}",
                afi, name, default, max_length
            )
        }?;
        writeln!(w, "!")?;
        Ok(())
    }
}

impl<W: Write> Formatter<W> for CiscoIos {
    fn write_prefix_set_ipv4(&self, set: &PrefixSet<Ipv4Prefix>, w: &mut W) -> Result<()> {
        self.write_prefix_set(set, w)
    }

    fn write_prefix_set_ipv6(&self, set: &PrefixSet<Ipv6Prefix>, w: &mut W) -> Result<()> {
        self.write_prefix_set(set, w)
    }
}
