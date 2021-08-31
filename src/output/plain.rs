use std::io::Write;

use anyhow::Result;
use prefixset::{IpPrefix, Ipv4Prefix, Ipv6Prefix, PrefixSet};

use super::Formatter;

#[derive(Default)]
pub struct Plain();

impl Plain {
    fn write_prefix_set<P, W>(&self, set: &PrefixSet<P>, w: &mut W) -> Result<()>
    where
        P: IpPrefix,
        W: Write,
    {
        set.prefixes()
            .try_for_each(|prefix| writeln!(w, "{}", prefix).map_err(|err| err.into()))
    }
}

impl<W: Write> Formatter<W> for Plain {
    fn write_prefix_set_ipv4(&self, set: &PrefixSet<Ipv4Prefix>, w: &mut W) -> Result<()> {
        self.write_prefix_set(set, w)
    }

    fn write_prefix_set_ipv6(&self, set: &PrefixSet<Ipv6Prefix>, w: &mut W) -> Result<()> {
        self.write_prefix_set(set, w)
    }
}

#[derive(Default)]
pub struct PlainRanges();

impl PlainRanges {
    fn write_prefix_set<P, W>(&self, set: &PrefixSet<P>, w: &mut W) -> Result<()>
    where
        P: IpPrefix,
        W: Write,
    {
        set.ranges()
            .try_for_each(|range| writeln!(w, "{}", range).map_err(|err| err.into()))
    }
}

impl<W: Write> Formatter<W> for PlainRanges {
    fn write_prefix_set_ipv4(&self, set: &PrefixSet<Ipv4Prefix>, w: &mut W) -> Result<()> {
        self.write_prefix_set(set, w)
    }

    fn write_prefix_set_ipv6(&self, set: &PrefixSet<Ipv6Prefix>, w: &mut W) -> Result<()> {
        self.write_prefix_set(set, w)
    }
}
